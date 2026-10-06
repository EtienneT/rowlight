//! Sliced Parquet reads, bounded summaries, and guarded whole-file viewing.

use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::source::{Cancellation, CsvOptions, Location, Source};
use anyhow::{Context, Result, ensure};
use polars_core::prelude::*;
#[cfg(test)]
use polars_io::prelude::*;

pub const PAGE_SIZES: [usize; 3] = [100, 500, 1_000];
pub const DEFAULT_PAGE_SIZE: usize = 500;
const HISTOGRAM_BINS: usize = 16;
pub const PROFILE_ROWS: usize = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "mode", content = "rows", rename_all = "snake_case")]
pub enum RowLimit {
    Paged(usize),
    All,
}

impl Default for RowLimit {
    fn default() -> Self {
        Self::Paged(DEFAULT_PAGE_SIZE)
    }
}
impl RowLimit {
    pub fn valid(self) -> bool {
        matches!(self, Self::All) || matches!(self, Self::Paged(size) if PAGE_SIZES.contains(&size))
    }
}

pub use crate::memory::all_rows_budget;

fn check_all_rows(estimate: u64, retained_bytes: u64, budget: u64) -> Result<()> {
    ensure!(
        estimate.saturating_add(retained_bytes) <= budget,
        "All rows would need approximately {:.0} MiB, above the safe budget of {:.0} MiB. Choose 100, 500, or 1000 rows per page.",
        estimate.saturating_add(retained_bytes) as f64 / (1u64 << 20) as f64,
        budget as f64 / (1u64 << 20) as f64
    );
    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub label: String,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Distribution {
    Histogram {
        bins: Vec<usize>,
        min: String,
        max: String,
        temporal: bool,
    },
    Categories(Vec<Bar>),
    None,
}

#[derive(Clone, Debug)]
pub struct ColumnProfile {
    pub name: String,
    pub dtype: String,
    pub null_count: usize,
    pub non_finite_count: usize,
    pub empty_count: usize,
    pub sortable: bool,
    pub summary_limited: bool,
    pub distribution: Distribution,
}

#[derive(Clone, Debug)]
pub struct DetailBar {
    pub axis: String,
    pub label: String,
    pub count: usize,
}

#[derive(Clone)]
pub struct ColumnDetail {
    pub profile: ColumnProfile,
    pub bars: Vec<DetailBar>,
    pub rows: usize,
    pub distinct: usize,
    pub distinct_available: bool,
    pub statistics: Vec<(String, String)>,
}

pub struct Page {
    pub source: Arc<Source>,
    pub path: PathBuf,
    pub total_rows: usize,
    pub file_bytes: u64,
    pub row_groups: usize,
    pub offset: usize,
    pub page_size: usize,
    pub row_limit: RowLimit,
    pub profile_rows: usize,
    pub frame: DataFrame,
    pub profiles: Vec<ColumnProfile>,
    /// Original zero-based row positions for a filtered page.
    pub source_rows: Option<Vec<usize>>,
    pub matching_rows: Option<usize>,
}

impl Page {
    /// More detail from the same bounded source sample, without reading more data.
    pub fn column_detail(&self, index: usize) -> Result<ColumnDetail> {
        let source = self
            .frame
            .columns()
            .get(index)
            .context("Column no longer exists")?;
        let column = source.slice(0, self.profile_rows);
        let profile = profile_with_options(&column, 32, 20)?;
        let unique = if profile.summary_limited {
            None
        } else {
            column.n_unique().ok()
        };
        let distinct = unique
            .unwrap_or_default()
            .saturating_sub(usize::from(column.null_count() > 0));
        let bars = match &profile.distribution {
            Distribution::Categories(bars) => bars
                .iter()
                .map(|bar| DetailBar {
                    axis: bar.label.clone(),
                    label: bar.label.clone(),
                    count: bar.count,
                })
                .collect(),
            Distribution::Histogram {
                bins,
                min: typed_min,
                ..
            } => {
                let numbers = if column.dtype().is_temporal() {
                    column.to_physical_repr()
                } else {
                    column.clone()
                }
                .cast(&DataType::Float64)?;
                let values = numbers.f64()?;
                let finite: Vec<_> = values.iter().flatten().filter(|v| v.is_finite()).collect();
                let min = finite.iter().copied().reduce(f64::min).unwrap_or(0.0);
                let max = finite.iter().copied().reduce(f64::max).unwrap_or(min);
                // Native ordering preserves exact integer and temporal labels.
                let indices = column.arg_sort(SortOptions {
                    nulls_last: true,
                    ..Default::default()
                });
                let mut ranges: Vec<Option<(String, String)>> = vec![None; bins.len()];
                for index in indices.no_null_iter() {
                    if let Some(value) = values.get(index as usize).filter(|v| v.is_finite()) {
                        let bin = bin_index(value, min, max, bins.len());
                        let text = preview(&column.get(index as usize)?, 80);
                        if let Some((_, last)) = &mut ranges[bin] {
                            *last = text;
                        } else {
                            ranges[bin] = Some((text.clone(), text));
                        }
                    }
                }
                bins.iter()
                    .enumerate()
                    .map(|(ix, &count)| DetailBar {
                        axis: if ix == 0 {
                            typed_min.clone()
                        } else {
                            let scale = min.abs().max(max.abs()).max(1.0);
                            let fraction = ix as f64 / bins.len() as f64;
                            let edge = ((1.0 - fraction) * (min / scale)
                                + fraction * (max / scale))
                                * scale;
                            bin_axis(edge, column.dtype())
                        },
                        label: ranges[ix].as_ref().map_or_else(
                            || format!("Bin {} · no observations", ix + 1),
                            |(low, high)| format!("Bin {} · observed {} – {}", ix + 1, low, high),
                        ),
                        count,
                    })
                    .collect()
            }
            Distribution::None => Vec::new(),
        };
        let statistics = if profile.summary_limited {
            Vec::new()
        } else {
            crate::inspection::statistics(&column)?
        };
        Ok(ColumnDetail {
            profile,
            bars,
            rows: column.len(),
            distinct,
            distinct_available: unique.is_some(),
            statistics,
        })
    }

    pub fn page_count(&self) -> usize {
        self.view_total_rows().div_ceil(self.page_size).max(1)
    }

    pub fn page_number(&self) -> usize {
        self.offset / self.page_size + 1
    }

    pub fn view_total_rows(&self) -> usize {
        self.matching_rows.unwrap_or(self.total_rows)
    }

    pub fn source_row(&self, row: usize) -> usize {
        self.source_rows
            .as_ref()
            .map_or(self.offset + row, |rows| rows[row])
    }

    /// Sort only this page, preserving source row numbers and exact integer ordering.
    pub fn sorted_rows(&self, column: usize, descending: bool) -> Result<Vec<usize>> {
        ensure!(
            self.profiles.get(column).is_some_and(|p| p.sortable),
            "This column cannot be sorted"
        );
        let indices = self.frame.columns()[column].arg_sort(SortOptions {
            descending,
            nulls_last: true,
            maintain_order: true,
            ..Default::default()
        });
        Ok(indices.no_null_iter().map(|i| i as usize).collect())
    }

    pub fn inspect_cell(&self, row: usize, column: usize) -> Result<CellDetail> {
        let source = self
            .frame
            .columns()
            .get(column)
            .context("Column no longer exists")?;
        let value = source.get(row)?;
        let (text, truncated) = format_value(&value, 65_536);
        Ok(CellDetail {
            name: source.name().to_string(),
            dtype: source.dtype().to_string(),
            source_row: self.source_row(row) + 1,
            text,
            truncated,
            null: matches!(value, AnyValue::Null),
        })
    }

    pub fn cell(&self, row: usize, column: usize) -> (String, bool) {
        match self
            .frame
            .columns()
            .get(column)
            .and_then(|c| c.get(row).ok())
        {
            Some(AnyValue::Null) => ("null".into(), true),
            Some(value) => (preview(&value, 256), false),
            None => (String::new(), false),
        }
    }
}

pub fn load_page(path: &Path, requested_offset: usize, page_size: usize) -> Result<Page> {
    load(path, requested_offset, RowLimit::Paged(page_size), 0)
}

pub fn load(
    path: &Path,
    requested_offset: usize,
    limit: RowLimit,
    retained_bytes: u64,
) -> Result<Page> {
    load_with_budget(
        path,
        requested_offset,
        limit,
        retained_bytes,
        all_rows_budget(),
    )
}

fn load_with_budget(
    path: &Path,
    requested_offset: usize,
    limit: RowLimit,
    retained_bytes: u64,
    budget: u64,
) -> Result<Page> {
    let cancel = Cancellation::default();
    let source = Source::open(
        Location::local(path.to_owned()),
        CsvOptions::default(),
        "",
        &cancel,
    )?;
    load_source_with_budget(
        source,
        requested_offset,
        limit,
        retained_bytes,
        budget,
        &cancel,
    )
}

pub fn load_source(
    source: Arc<Source>,
    requested_offset: usize,
    limit: RowLimit,
    retained_bytes: u64,
    cancel: &Cancellation,
) -> Result<Page> {
    load_source_with_budget(
        source,
        requested_offset,
        limit,
        retained_bytes,
        all_rows_budget(),
        cancel,
    )
}
fn load_source_with_budget(
    source: Arc<Source>,
    requested_offset: usize,
    limit: RowLimit,
    retained_bytes: u64,
    budget: u64,
    cancel: &Cancellation,
) -> Result<Page> {
    ensure!(
        limit.valid(),
        "Page size must be 100, 500, or 1000 rows, or All"
    );
    let total_rows = source.info.total_rows;
    let page_size = match limit {
        RowLimit::Paged(size) => size,
        RowLimit::All => total_rows.max(1),
    };
    let last_offset = total_rows.saturating_sub(1) / page_size * page_size;
    let offset = (requested_offset / page_size * page_size).min(last_offset);
    if limit == RowLimit::All {
        let estimate = source.allocation_estimate(1);
        check_all_rows(estimate, retained_bytes, budget)?;
        let sample = source.read_slice(0, PROFILE_ROWS, cancel)?;
        let estimate = (sample.estimated_size() as u64)
            .saturating_mul(total_rows as u64)
            .checked_div(sample.height().max(1) as u64)
            .unwrap_or(u64::MAX)
            .saturating_mul(4)
            .saturating_add(32 << 20)
            .max(source.allocation_estimate(sample.width()));
        check_all_rows(estimate, retained_bytes, budget)?;
    }
    let frame = source.read_slice(offset, page_size, cancel)?;
    make_page(source, offset, page_size, limit, frame)
}
pub(crate) fn make_page(
    source: Arc<Source>,
    offset: usize,
    page_size: usize,
    limit: RowLimit,
    frame: DataFrame,
) -> Result<Page> {
    let profile_rows = frame.height().min(PROFILE_ROWS);
    let profiles = frame
        .slice(0, profile_rows)
        .columns()
        .iter()
        .map(profile)
        .collect::<Result<_>>()?;
    let path = match &source.location {
        Location::Local(path) => path.clone(),
        Location::Remote(_) => PathBuf::from(source.location.display()),
    };
    Ok(Page {
        path,
        total_rows: source.info.total_rows,
        file_bytes: source.info.file_bytes,
        row_groups: source.info.groups.len(),
        source,
        offset,
        page_size,
        row_limit: limit,
        profile_rows,
        frame,
        profiles,
        source_rows: None,
        matching_rows: None,
    })
}

pub struct FileOverview {
    pub page: Page,
    pub windows: usize,
    pub details: Vec<ColumnDetail>,
}
/// Deterministic windows spread across the source, capped at 1,000 rows in total.
/// This is a stratified preview, not a random sample or exact global statistics.
pub fn overview(source: Arc<Source>, cancel: &Cancellation) -> Result<FileOverview> {
    let total = source.info.total_rows;
    let windows = if total <= PROFILE_ROWS { 1 } else { 16 };
    let count = if windows == 1 {
        total
    } else {
        PROFILE_ROWS / windows
    };
    let mut frame = source.read_slice(0, count, cancel)?;
    ensure!(
        frame.estimated_size() as u64 <= all_rows_budget() / 4,
        "Overview sample exceeds the safe memory budget"
    );
    for window in 1..windows {
        cancel.check()?;
        let offset = ((total - count) as u128 * window as u128 / (windows - 1) as u128) as usize;
        frame.vstack_mut(&source.read_slice(offset, count, cancel)?)?;
        ensure!(
            frame.estimated_size() as u64 <= all_rows_budget() / 4,
            "Overview sample exceeds the safe memory budget"
        );
    }
    let page = make_page(
        source,
        0,
        PROFILE_ROWS,
        RowLimit::Paged(PROFILE_ROWS),
        frame,
    )?;
    let mut details = Vec::with_capacity(page.profiles.len());
    for column in 0..page.profiles.len() {
        cancel.check()?;
        details.push(page.column_detail(column)?);
    }
    Ok(FileOverview {
        page,
        windows,
        details,
    })
}

fn profile(column: &Column) -> Result<ColumnProfile> {
    profile_with_options(column, HISTOGRAM_BINS, 4)
}

fn profile_with_options(column: &Column, bins: usize, categories: usize) -> Result<ColumnProfile> {
    let dtype = column.dtype();
    let mut result = ColumnProfile {
        name: column.name().to_string(),
        dtype: dtype.to_string(),
        null_count: column.null_count(),
        non_finite_count: 0,
        empty_count: 0,
        sortable: dtype.is_numeric()
            || dtype.is_temporal()
            || matches!(
                dtype,
                DataType::String
                    | DataType::Boolean
                    | DataType::Categorical(..)
                    | DataType::Enum(..)
            ),
        summary_limited: column.as_materialized_series().estimated_size() > 8 << 20,
        distribution: Distribution::None,
    };
    if result.summary_limited || column.null_count() == column.len() {
        return Ok(result);
    }
    if dtype.is_numeric() || dtype.is_temporal() {
        let physical = if dtype.is_temporal() {
            column.to_physical_repr()
        } else {
            column.clone()
        };
        let numbers = physical.cast(&DataType::Float64)?;
        let values: Vec<f64> = numbers
            .f64()?
            .iter()
            .flatten()
            .filter(|v| {
                if v.is_finite() {
                    true
                } else {
                    result.non_finite_count += 1;
                    false
                }
            })
            .collect();
        result.distribution = histogram_with_bins(&values, dtype.is_temporal(), bins);
        // Show actual typed endpoints (dates, decimal scale and full integer precision).
        if let Distribution::Histogram { min, max, .. } = &mut result.distribution {
            let indices = column.arg_sort(SortOptions {
                nulls_last: true,
                ..Default::default()
            });
            let finite_indices: Vec<_> = indices
                .no_null_iter()
                .filter(|&i| {
                    numbers
                        .f64()
                        .ok()
                        .and_then(|c| c.get(i as usize))
                        .is_some_and(f64::is_finite)
                })
                .collect();
            if let (Some(&first), Some(&last)) = (finite_indices.first(), finite_indices.last()) {
                *min = preview(&column.get(first as usize)?, 40);
                *max = preview(&column.get(last as usize)?, 40);
            }
        }
    } else if matches!(dtype, DataType::Boolean) {
        let mut counts = [0, 0];
        for value in column.bool()?.iter().flatten() {
            counts[usize::from(value)] += 1;
        }
        result.distribution = Distribution::Categories(vec![
            Bar {
                label: "false".into(),
                count: counts[0],
            },
            Bar {
                label: "true".into(),
                count: counts[1],
            },
        ]);
    } else if matches!(
        dtype,
        DataType::String | DataType::Categorical(..) | DataType::Enum(..)
    ) {
        let strings = column.cast(&DataType::String)?;
        let mut counts = BTreeMap::new();
        for value in strings.str()?.iter().flatten() {
            result.empty_count += usize::from(value.is_empty());
            *counts.entry(value).or_insert(0_usize) += 1;
        }
        let mut counts: Vec<_> = counts.into_iter().collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let other_count = counts.iter().skip(categories).map(|(_, n)| n).sum();
        let mut bars: Vec<_> = counts
            .into_iter()
            .take(categories)
            .map(|(label, count)| Bar {
                label: if label.is_empty() {
                    "(empty)".into()
                } else {
                    label
                        .chars()
                        .take(if categories > 4 { 256 } else { 40 })
                        .collect()
                },
                count,
            })
            .collect();
        if other_count > 0 {
            bars.push(Bar {
                label: "Other".into(),
                count: other_count,
            });
        }
        result.distribution = Distribution::Categories(bars);
    }
    Ok(result)
}

#[cfg(test)]
fn histogram(values: &[f64], temporal: bool) -> Distribution {
    histogram_with_bins(values, temporal, HISTOGRAM_BINS)
}

fn bin_index(value: f64, min: f64, max: f64, bins: usize) -> usize {
    let scale = min.abs().max(max.abs()).max(1.0);
    let low = min / scale;
    let span = max / scale - low;
    if span == 0.0 {
        0
    } else {
        (((value / scale - low) / span * bins as f64) as usize).min(bins - 1)
    }
}

fn histogram_with_bins(values: &[f64], temporal: bool, bin_count: usize) -> Distribution {
    let Some(min) = values.iter().copied().reduce(f64::min) else {
        return Distribution::None;
    };
    let max = values.iter().copied().reduce(f64::max).unwrap_or(min);
    let mut bins = vec![0; if min == max { 1 } else { bin_count }];
    // Scale before subtracting to avoid overflow for ranges such as -1e308..1e308.
    for &value in values {
        let index = bin_index(value, min, max, bins.len());
        bins[index] += 1;
    }
    Distribution::Histogram {
        bins,
        min: min.to_string(),
        max: max.to_string(),
        temporal,
    }
}

fn bin_axis(value: f64, dtype: &DataType) -> String {
    // Temporal coordinates use the original unit/timezone; bin edges are approximate.
    let scalar = Series::new("".into(), [value]).into_column();
    let typed = if dtype.is_temporal() {
        scalar
            .cast(&dtype.to_physical())
            .and_then(|c| c.cast(dtype))
    } else {
        scalar.cast(dtype)
    };
    let label = typed
        .ok()
        .and_then(|c| {
            c.get(0).ok().map(|v| {
                if v.is_null() {
                    value.to_string()
                } else {
                    preview(&v, 80)
                }
            })
        })
        .unwrap_or_else(|| value.to_string());
    format!("≈{label}")
}

pub struct CellDetail {
    pub name: String,
    pub dtype: String,
    pub source_row: usize,
    pub text: String,
    pub truncated: bool,
    pub null: bool,
}

/// Bound formatting even for long strings, binary values and nested columns.
fn preview(value: &AnyValue<'_>, limit: usize) -> String {
    format_value(value, limit).0
}
pub(crate) fn format_value(value: &AnyValue<'_>, limit: usize) -> (String, bool) {
    struct Limited {
        text: String,
        remaining: usize,
    }
    impl fmt::Write for Limited {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let mut chars = s.chars();
            for c in chars.by_ref().take(self.remaining) {
                self.text.push(c);
                self.remaining -= 1;
            }
            if chars.next().is_some() {
                Err(fmt::Error)
            } else {
                Ok(())
            }
        }
    }
    let mut output = Limited {
        text: String::new(),
        remaining: limit,
    };
    let truncated = match value {
        AnyValue::String(s) => fmt::Write::write_str(&mut output, s).is_err(),
        AnyValue::StringOwned(s) => fmt::Write::write_str(&mut output, s.as_str()).is_err(),
        _ => write_nested(&mut output, value, 0).is_err(),
    };
    if truncated {
        output.text.push('…');
    }
    (output.text, truncated)
}

fn write_nested(output: &mut impl fmt::Write, value: &AnyValue<'_>, depth: usize) -> fmt::Result {
    if depth > 32 {
        return Err(fmt::Error);
    }
    match value {
        AnyValue::List(series) | AnyValue::Array(series, _) => {
            output.write_str("[")?;
            for row in 0..series.len() {
                if row > 0 {
                    output.write_str(", ")?;
                }
                write_nested(output, &series.get(row).map_err(|_| fmt::Error)?, depth + 1)?;
            }
            output.write_str("]")
        }
        AnyValue::Struct(row, array, _) => {
            output.write_str("{")?;
            for (index, (field, values)) in array.fields().iter().zip(array.values()).enumerate() {
                if index > 0 {
                    output.write_str(", ")?;
                }
                write!(output, "{:?}: ", field.name.as_str())?;
                let series = Series::try_from((field, values.clone())).map_err(|_| fmt::Error)?;
                write_nested(
                    output,
                    &series.get(*row).map_err(|_| fmt::Error)?,
                    depth + 1,
                )?;
            }
            output.write_str("}")
        }
        AnyValue::StructOwned(values) => {
            output.write_str("{")?;
            for (index, (value, field)) in values.0.iter().zip(&values.1).enumerate() {
                if index > 0 {
                    output.write_str(", ")?;
                }
                write!(output, "{:?}: ", field.name().as_str())?;
                write_nested(output, value, depth + 1)?;
            }
            output.write_str("}")
        }
        AnyValue::Binary(bytes) => {
            output.write_str("0x")?;
            for byte in *bytes {
                write!(output, "{byte:02x}")?;
            }
            Ok(())
        }
        AnyValue::BinaryOwned(bytes) => {
            output.write_str("0x")?;
            for byte in bytes {
                write!(output, "{byte:02x}")?;
            }
            Ok(())
        }
        AnyValue::String(text) if depth > 0 => write!(output, "{text:?}"),
        AnyValue::StringOwned(text) if depth > 0 => write!(output, "{:?}", text.as_str()),
        _ => write!(output, "{value}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspector_formats_complete_nested_binary_values_and_bounds_large_text() -> Result<()> {
        let list = Series::new(
            "items".into(),
            [Series::new("".into(), (0..100i64).collect::<Vec<_>>())],
        );
        let (text, truncated) = format_value(&list.get(0)?, 65_536);
        assert!(!truncated);
        assert!(text.ends_with("98, 99]"));
        assert_eq!(text.matches(", ").count(), 99);
        let (text, truncated) = format_value(&AnyValue::Binary(&[0, 15, 255]), 65_536);
        assert_eq!(text, "0x000fff");
        assert!(!truncated);
        let value = "é".repeat(70_000);
        let (text, truncated) = format_value(&AnyValue::String(&value), 65_536);
        assert!(truncated);
        assert_eq!(text.chars().count(), 65_537);
        Ok(())
    }

    #[test]
    fn detailed_profiles_keep_the_sample_bounded_and_account_for_every_value() -> Result<()> {
        let mut frame = df!(
            "integer" => (0..2501u64).map(|v| u64::MAX - v).collect::<Vec<_>>(),
            "category" => (0..2501).map(|v| format!("category_{}", v % 30)).collect::<Vec<_>>(),
            "flag" => (0..2501).map(|v| if v % 3 == 0 { None } else { Some(v % 2 == 0) }).collect::<Vec<_>>()
        )?;
        let file = fixture(&mut frame)?;
        let page = load_with_budget(file.path(), 0, RowLimit::All, 0, 1 << 30)?;
        let numeric = page.column_detail(0)?;
        assert_eq!(numeric.rows, PROFILE_ROWS);
        assert_eq!(numeric.distinct, PROFILE_ROWS);
        assert_eq!(
            numeric.bars.iter().map(|b| b.count).sum::<usize>(),
            PROFILE_ROWS
        );
        let Distribution::Histogram { max, .. } = numeric.profile.distribution else {
            panic!("Expected histogram");
        };
        assert_eq!(max, u64::MAX.to_string());
        assert!(
            numeric
                .bars
                .iter()
                .any(|b| b.label.contains(&u64::MAX.to_string()))
        );
        let categorical = page.column_detail(1)?;
        assert_eq!(categorical.distinct, 30);
        assert_eq!(categorical.bars.len(), 21);
        assert_eq!(categorical.bars.last().unwrap().axis, "Other");
        assert_eq!(
            categorical.bars.iter().map(|b| b.count).sum::<usize>(),
            PROFILE_ROWS
        );
        let boolean = page.column_detail(2)?;
        assert_eq!(boolean.bars.len(), 2);
        assert_eq!(boolean.distinct, 2);
        assert_eq!(
            boolean.bars.iter().map(|b| b.count).sum::<usize>() + boolean.profile.null_count,
            PROFILE_ROWS
        );
        assert!(page.column_detail(99).is_err());
        Ok(())
    }

    #[test]
    fn all_rows_loads_entire_file_and_rejects_unsafe_reads() -> Result<()> {
        let mut frame = DataFrame::new(
            2501,
            vec![Series::new("x".into(), (0..2501i64).collect::<Vec<_>>()).into_column()],
        )?;
        let file = fixture(&mut frame)?;
        let page = load_with_budget(file.path(), 2000, RowLimit::All, 0, 1 << 30)?;
        assert_eq!(page.frame.height(), 2501);
        assert_eq!(page.offset, 0);
        assert_eq!(page.page_count(), 1);
        assert_eq!(page.profile_rows, PROFILE_ROWS);
        assert_eq!(page.sorted_rows(0, true)?[0], 2500);
        assert!(
            load_with_budget(file.path(), 0, RowLimit::All, 0, 1 << 20)
                .err()
                .unwrap()
                .to_string()
                .contains("safe budget")
        );
        assert!(load_with_budget(file.path(), 0, RowLimit::All, 1 << 30, 1 << 30).is_err());
        let paged = load_with_budget(file.path(), 0, RowLimit::Paged(100), 0, 0)?;
        assert_eq!(paged.frame.height(), 100);
        Ok(())
    }

    fn fixture(frame: &mut DataFrame) -> Result<tempfile::NamedTempFile> {
        let mut file = tempfile::NamedTempFile::new()?;
        ParquetWriter::new(file.as_file_mut())
            .with_row_group_size(Some(73))
            .finish(frame)?;
        Ok(file)
    }

    #[test]
    fn paging_crosses_row_groups_and_clamps_last_page() -> Result<()> {
        let mut frame = df!("id" => (0..251_i64).collect::<Vec<_>>(), "flag" => vec![true; 251])?;
        let file = fixture(&mut frame)?;
        let page = load_page(file.path(), 100, 100)?;
        assert_eq!(
            (
                page.total_rows,
                page.offset,
                page.frame.height(),
                page.page_count()
            ),
            (251, 100, 100, 3)
        );
        assert_eq!(page.cell(0, 0).0, "100");
        assert_eq!(page.cell(99, 0).0, "199");
        assert!(page.row_groups > 1);
        let last = load_page(file.path(), usize::MAX, 100)?;
        assert_eq!((last.offset, last.frame.height()), (200, 51));
        Ok(())
    }

    #[test]
    fn empty_file_retains_schema() -> Result<()> {
        let mut frame = df!("id" => Vec::<i64>::new(), "text" => Vec::<String>::new())?;
        let file = fixture(&mut frame)?;
        let page = load_page(file.path(), 0, 500)?;
        assert_eq!(
            (
                page.total_rows,
                page.frame.height(),
                page.profiles.len(),
                page.page_count()
            ),
            (0, 0, 2, 1)
        );
        Ok(())
    }

    #[test]
    fn sort_uses_native_integer_precision_and_nulls_last() -> Result<()> {
        let mut frame = df!("large" => [Some(u64::MAX), Some(u64::MAX - 1), None, Some(1)])?;
        let file = fixture(&mut frame)?;
        let page = load_page(file.path(), 0, 100)?;
        assert_eq!(page.sorted_rows(0, false)?, [3, 1, 0, 2]);
        assert_eq!(page.sorted_rows(0, true)?, [0, 1, 3, 2]);
        assert_eq!(page.profiles[0].null_count, 1);
        Ok(())
    }

    #[test]
    fn plots_account_for_nulls_nonfinite_and_categories() -> Result<()> {
        let mut frame = df!(
            "float" => [Some(-1e308), Some(1e308), Some(f64::NAN), Some(f64::INFINITY), None, Some(0.0)],
            "bool" => [Some(true), Some(false), None, Some(true), Some(true), Some(false)],
            "text" => ["a", "b", "c", "d", "e", "a"]
        )?;
        let file = fixture(&mut frame)?;
        let page = load_page(file.path(), 0, 100)?;
        assert_eq!(page.profiles[0].non_finite_count, 2);
        let Distribution::Histogram { bins, .. } = &page.profiles[0].distribution else {
            panic!("expected histogram");
        };
        assert_eq!(bins.iter().sum::<usize>(), 3);
        assert_eq!((bins[0], bins[15]), (1, 1));
        let Distribution::Categories(bars) = &page.profiles[1].distribution else {
            panic!("expected boolean bars");
        };
        assert_eq!((bars[0].count, bars[1].count), (2, 3));
        let Distribution::Categories(bars) = &page.profiles[2].distribution else {
            panic!("expected category bars");
        };
        assert_eq!(bars.iter().map(|b| b.count).sum::<usize>(), 6);
        assert_eq!(bars.last().map(|b| b.label.as_str()), Some("Other"));
        Ok(())
    }

    #[test]
    fn constant_and_all_null_columns_are_safe() -> Result<()> {
        assert!(matches!(histogram(&[], false), Distribution::None));
        let Distribution::Histogram { bins, .. } = histogram(&[3.0; 10], false) else {
            panic!("histogram");
        };
        assert_eq!(bins, [10]);
        let column = Series::new("nulls".into(), &[None::<f64>, None]).into_column();
        assert_eq!(profile(&column)?.null_count, 2);
        assert!(matches!(profile(&column)?.distribution, Distribution::None));
        Ok(())
    }

    #[test]
    fn rejects_bad_input_and_bounds_unicode_preview() -> Result<()> {
        let file = tempfile::NamedTempFile::new()?;
        assert!(load_page(file.path(), 0, 100).is_err());
        assert!(load_page(file.path(), 0, 0).is_err());
        assert_eq!(preview(&AnyValue::String("é你好"), 2), "é你…");
        Ok(())
    }

    #[test]
    fn dates_decimals_and_nested_columns_round_trip() -> Result<()> {
        let date = Series::new("date".into(), [20_000_i32, 20_001]).cast(&DataType::Date)?;
        let decimal =
            Series::new("price".into(), [12.5_f64, 19.75]).cast(&DataType::Decimal(10, 2))?;
        let list = Series::new(
            "items".into(),
            [
                Series::new("".into(), [1_i64, 2]),
                Series::new("".into(), [3_i64]),
            ],
        );
        let binary = Series::new("bytes".into(), &[b"abc".as_slice(), b"def".as_slice()]);
        let mut frame = DataFrame::new(
            2,
            vec![
                date.into_column(),
                decimal.into_column(),
                list.into_column(),
                binary.into_column(),
            ],
        )?;
        let file = fixture(&mut frame)?;
        let page = load_page(file.path(), 0, 100)?;
        assert_eq!(page.profiles[0].dtype, "date");
        assert!(matches!(
            page.profiles[0].distribution,
            Distribution::Histogram { temporal: true, .. }
        ));
        assert_eq!(page.cell(0, 0).0, "2024-10-04");
        assert_eq!(page.cell(0, 1).0, "12.50");
        let detailed_date = page.column_detail(0)?;
        assert_eq!(detailed_date.bars[0].axis, "2024-10-04");
        assert!(detailed_date.bars[1].axis.starts_with("≈2024-10-04"));
        assert_eq!(
            detailed_date
                .bars
                .iter()
                .map(|bar| bar.count)
                .sum::<usize>(),
            2
        );
        assert_eq!(page.column_detail(1)?.bars[0].axis, "12.50");
        assert!(!page.profiles[2].sortable);
        assert!(matches!(page.profiles[2].distribution, Distribution::None));
        assert!(page.cell(0, 2).0.contains('1'));
        assert_eq!(page.profiles[3].dtype, "binary");
        Ok(())
    }

    // APFS accepts only valid UTF-8 filenames; exercise raw byte names on Linux.
    // https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/APFS_Guide/FAQ/FAQ.html
    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_paths_are_supported() -> Result<()> {
        use std::{fs::File, os::unix::ffi::OsStringExt};
        let dir = tempfile::tempdir()?;
        let path = dir
            .path()
            .join(std::ffi::OsString::from_vec(b"file-\xff.parquet".to_vec()));
        let mut frame = df!("value" => [1_i64])?;
        ParquetWriter::new(File::create(&path)?).finish(&mut frame)?;
        assert_eq!(load_page(&path, 0, 100)?.cell(0, 0).0, "1");
        Ok(())
    }
}
