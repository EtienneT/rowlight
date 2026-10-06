//! Read-only filters: bounded batch scans and a temporary disk-backed row index.
use crate::{
    data::{self, Page, RowLimit},
    source::{Cancellation, Source},
};
use anyhow::{Context, Result, ensure};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use polars_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
const SCAN_ROWS: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Contains,
    #[default]
    Equals,
    Range,
    IsNull,
    NotNull,
}
impl FilterOp {
    pub fn label(self) -> &'static str {
        match self {
            Self::Contains => "Contains",
            Self::Equals => "Equals",
            Self::Range => "Between (inclusive)",
            Self::IsNull => "Is null",
            Self::NotNull => "Is not null",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Filter {
    pub column: String,
    pub op: FilterOp,
    pub value: String,
    pub upper: String,
}
impl Filter {
    pub fn label(&self) -> String {
        match self.op {
            FilterOp::IsNull | FilterOp::NotNull => {
                format!("{} {}", self.column, self.op.label().to_lowercase())
            }
            FilterOp::Range => format!("{}: {} – {}", self.column, self.value, self.upper),
            _ => format!(
                "{} {} {:?}",
                self.column,
                self.op.label().to_lowercase(),
                self.value
            ),
        }
    }
}
struct Predicate {
    column: String,
    op: FilterOp,
    lower: AnyValue<'static>,
    upper: AnyValue<'static>,
    text: bool,
}
fn bound(value: &str, dtype: &DataType) -> Result<AnyValue<'static>> {
    let value_trim = value.trim();
    let invalid = || anyhow::anyhow!("Enter a valid {dtype} value");
    let v = match dtype {
        DataType::String | DataType::Categorical(..) | DataType::Enum(..) => {
            AnyValue::StringOwned(value.into())
        }
        DataType::Boolean => AnyValue::Boolean(match value_trim.to_ascii_lowercase().as_str() {
            "true" => true,
            "false" => false,
            _ => return Err(invalid()),
        }),
        DataType::Date => {
            let date = NaiveDate::parse_from_str(value_trim, "%Y-%m-%d").map_err(|_| invalid())?;
            AnyValue::Date(i32::try_from(
                date.signed_duration_since(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
                    .num_days(),
            )?)
        }
        DataType::Datetime(unit, tz) => {
            let date = chrono::DateTime::parse_from_rfc3339(value_trim)
                .map(|d| d.with_timezone(&chrono::Utc))
                .or_else(|_| {
                    NaiveDateTime::parse_from_str(value_trim, "%Y-%m-%d %H:%M:%S%.f")
                        .map(|d| d.and_utc())
                })
                .map_err(|_| {
                    anyhow::anyhow!(
                        "Use an ISO timestamp with timezone, or YYYY-MM-DD HH:MM:SS (UTC)"
                    )
                })?;
            let precision = match unit {
                TimeUnit::Milliseconds => 1_000_000,
                TimeUnit::Microseconds => 1_000,
                TimeUnit::Nanoseconds => 1,
            };
            ensure!(
                date.timestamp_subsec_nanos() % precision == 0,
                "The timestamp has finer precision than this column stores"
            );
            let raw = match unit {
                TimeUnit::Milliseconds => date.timestamp_millis(),
                TimeUnit::Microseconds => date.timestamp_micros(),
                TimeUnit::Nanoseconds => date
                    .timestamp_nanos_opt()
                    .context("Timestamp is outside nanosecond range")?,
            };
            AnyValue::DatetimeOwned(raw, *unit, tz.clone().map(Arc::new))
        }
        DataType::Time => {
            let time = NaiveTime::parse_from_str(value_trim, "%H:%M:%S%.f")
                .map_err(|_| anyhow::anyhow!("Use HH:MM:SS[.fraction]"))?;
            AnyValue::Time(
                time.num_seconds_from_midnight() as i64 * 1_000_000_000 + time.nanosecond() as i64,
            )
        }
        DataType::Duration(unit) => {
            AnyValue::Duration(value_trim.parse().map_err(|_| invalid())?, *unit)
        }
        dtype if dtype.is_numeric() => {
            if let DataType::Decimal(_, scale) = dtype
                && let Some((_, fraction)) = value_trim.split_once('.')
            {
                ensure!(
                    fraction.chars().skip(*scale).all(|c| c == '0'),
                    "The decimal has more fractional digits than this column stores"
                );
            }
            let c = Column::new("bound".into(), [value_trim])
                .cast(dtype)
                .map_err(|_| invalid())?;
            let v = c.get(0)?.into_static();
            ensure!(!v.is_null(), "Enter a valid {dtype} value");
            ensure!(
                !matches!(v,AnyValue::Float64(n) if !n.is_finite())
                    && !matches!(v,AnyValue::Float32(n) if !n.is_finite()),
                "Use a finite numeric bound"
            );
            v
        }
        _ => return Err(anyhow::anyhow!("Use null/not-null for this column type")),
    };
    Ok(v)
}
impl Predicate {
    fn compile(filter: &Filter, frame: &DataFrame) -> Result<Self> {
        ensure!(
            filter.value.len() <= 4096 && filter.upper.len() <= 4096,
            "Filter values must be at most 4 KiB"
        );
        let column = anyhow::Context::context(
            frame.column(&filter.column),
            "Filter column is no longer in this file",
        )?;
        let text = matches!(
            column.dtype(),
            DataType::String | DataType::Categorical(..) | DataType::Enum(..)
        );
        let (lower, upper) = match filter.op {
            FilterOp::IsNull | FilterOp::NotNull => (AnyValue::Null, AnyValue::Null),
            FilterOp::Contains => {
                ensure!(text, "Contains requires a text column");
                (bound(&filter.value, column.dtype())?, AnyValue::Null)
            }
            FilterOp::Equals => (bound(&filter.value, column.dtype())?, AnyValue::Null),
            FilterOp::Range => {
                ensure!(
                    column.dtype().is_numeric() || column.dtype().is_temporal(),
                    "Ranges require a numeric or date/time column"
                );
                let low = bound(&filter.value, column.dtype())?;
                let high = bound(&filter.upper, column.dtype())?;
                ensure!(
                    low.partial_cmp(&high).is_some_and(|o| o.is_le()),
                    "The lower bound must not exceed the upper bound"
                );
                (low, high)
            }
        };
        Ok(Self {
            column: filter.column.clone(),
            op: filter.op,
            lower,
            upper,
            text,
        })
    }
    fn matches(&self, value: AnyValue<'_>) -> bool {
        match self.op {
            FilterOp::IsNull => value.is_null(),
            FilterOp::NotNull => !value.is_null(),
            _ if value.is_null() => false,
            FilterOp::Contains => value
                .get_str()
                .is_some_and(|s| s.contains(self.lower.get_str().unwrap())),
            FilterOp::Equals => value == self.lower,
            FilterOp::Range => {
                value.partial_cmp(&self.lower).is_some_and(|o| o.is_ge())
                    && value.partial_cmp(&self.upper).is_some_and(|o| o.is_le())
            }
        }
    }
}
pub fn validate(filters: &[Filter], frame: &DataFrame) -> Result<()> {
    ensure!(filters.len() <= 8, "Use at most eight column filters");
    for f in filters {
        Predicate::compile(f, frame)?;
    }
    Ok(())
}

pub struct FilterIndex {
    pub source: Arc<Source>,
    pub filters: Vec<Filter>,
    pub matches: usize,
    index: tempfile::NamedTempFile,
}
impl FilterIndex {
    pub fn build(
        source: Arc<Source>,
        filters: Vec<Filter>,
        cancel: &Cancellation,
        progress: &AtomicUsize,
    ) -> Result<Self> {
        ensure!(
            !filters.is_empty() && filters.len() <= 8,
            "Choose one to eight filters"
        );
        let index = tempfile::NamedTempFile::new().context("Cannot create filter row index")?;
        let mut writer = BufWriter::new(index.reopen()?);
        let mut matches = 0usize;
        for offset in (0..source.info.total_rows.max(1)).step_by(SCAN_ROWS) {
            cancel.check()?;
            let frame = source.read_slice(offset, SCAN_ROWS, cancel)?;
            ensure!(
                frame.estimated_size() as u64 <= data::all_rows_budget() / 4,
                "Filter batch exceeds the safe memory budget"
            );
            let predicates = filters
                .iter()
                .map(|f| Predicate::compile(f, &frame))
                .collect::<Result<Vec<_>>>()?;
            let columns = predicates
                .iter()
                .map(|p| {
                    let c = frame.column(&p.column)?;
                    if p.text {
                        c.cast(&DataType::String)
                    } else {
                        Ok(c.clone())
                    }
                })
                .collect::<PolarsResult<Vec<_>>>()?;
            for row in 0..frame.height() {
                if row % 256 == 0 {
                    cancel.check()?;
                }
                let mut keep = true;
                for (predicate, column) in predicates.iter().zip(&columns) {
                    if !predicate.matches(column.get(row)?) {
                        keep = false;
                        break;
                    }
                }
                if keep {
                    writer.write_all(&((offset + row) as u64).to_le_bytes())?;
                    matches += 1;
                }
            }
            progress.store(
                (offset + frame.height()).min(source.info.total_rows),
                Ordering::Release,
            );
        }
        writer.flush()?;
        source.check_unchanged()?;
        cancel.check()?;
        Ok(Self {
            source,
            filters,
            matches,
            index,
        })
    }
    pub fn load(
        &self,
        requested: usize,
        limit: RowLimit,
        retained: u64,
        cancel: &Cancellation,
    ) -> Result<Page> {
        ensure!(limit.valid(), "Choose a supported page size");
        let count = match limit {
            RowLimit::Paged(n) => n,
            RowLimit::All => self.matches.max(1),
        };
        let offset =
            (requested / count * count).min(self.matches.saturating_sub(1) / count * count);
        let count_rows = count.min(self.matches.saturating_sub(offset));
        if limit == RowLimit::All && self.matches > 0 {
            // Conservatively account for value decoding and the retained old page.
            ensure!(
                self.source.allocation_estimate(1).saturating_add(retained)
                    <= data::all_rows_budget(),
                "All filtered rows exceed the safe budget; use paging"
            );
        }
        ensure!(
            (count_rows as u64)
                .saturating_mul(16)
                .saturating_add(retained)
                <= data::all_rows_budget(),
            "Filtered row index exceeds the safe budget; use paging"
        );
        let mut file: File = self.index.reopen()?;
        file.seek(SeekFrom::Start((offset as u64).saturating_mul(8)))?;
        let mut ids = Vec::with_capacity(count_rows.min(1000));
        for _ in 0..count_rows {
            let mut bytes = [0; 8];
            file.read_exact(&mut bytes)?;
            ids.push(usize::try_from(u64::from_le_bytes(bytes))?);
        }
        let mut result: Option<DataFrame> = None;
        let mut at = 0;
        while at < ids.len() {
            cancel.check()?;
            let start = ids[at];
            let end = ids.partition_point(|&id| id < start.saturating_add(SCAN_ROWS));
            let batch = self
                .source
                .read_slice(start, ids[end - 1] - start + 1, cancel)?;
            let indices = IdxCa::from_vec(
                "rows".into(),
                ids[at..end]
                    .iter()
                    .map(|&id| (id - start) as IdxSize)
                    .collect(),
            );
            let selected = batch.take(&indices)?;
            if let Some(frame) = &mut result {
                frame.vstack_mut(&selected)?;
            } else {
                result = Some(selected);
            }
            ensure!(
                (result.as_ref().unwrap().estimated_size() as u64)
                    .saturating_mul(4)
                    .saturating_add(retained)
                    <= data::all_rows_budget(),
                "Filtered page exceeds the safe memory budget; select a smaller page"
            );
            at = end;
        }
        let frame = match result {
            Some(frame) => frame,
            None => self.source.read_slice(0, 0, cancel)?,
        };
        let mut page = data::make_page(Arc::clone(&self.source), offset, count, limit, frame)?;
        page.source_rows = Some(ids);
        page.matching_rows = Some(self.matches);
        Ok(page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars_io::prelude::*;
    #[test]
    fn filters_preserve_integer_precision_nulls_and_temporal_types() -> Result<()> {
        let frame = df!("id"=>&[u64::MAX,u64::MAX-1], "s"=>&[Some(""),None])?;
        let f = Filter {
            column: "id".into(),
            op: FilterOp::Equals,
            value: u64::MAX.to_string(),
            upper: String::new(),
        };
        let p = Predicate::compile(&f, &frame)?;
        assert!(p.matches(frame.column("id")?.get(0)?));
        assert!(!p.matches(frame.column("id")?.get(1)?));
        assert_eq!(bound("2026-10-05", &DataType::Date)?, AnyValue::Date(20731));
        assert_eq!(
            bound("12:00:00", &DataType::Time)?,
            AnyValue::Time(43_200_000_000_000)
        );
        assert!(bound("NaN", &DataType::Float64).is_err());
        assert!(bound("2026-02-30", &DataType::Date).is_err());
        Ok(())
    }
    #[test]
    fn disk_index_pages_sparse_matches_with_original_rows_and_cancels() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("data.parquet");
        let mut frame = df!("id"=> (0..10_000i64).collect::<Vec<_>>())?;
        ParquetWriter::new(File::create(&path)?)
            .with_row_group_size(Some(1024))
            .finish(&mut frame)?;
        let source = Source::open(
            crate::source::Location::local(path),
            Default::default(),
            "",
            &Cancellation::default(),
        )?;
        let f = Filter {
            column: "id".into(),
            op: FilterOp::Range,
            value: "8500".into(),
            upper: "9500".into(),
        };
        let progress = AtomicUsize::new(0);
        let index = FilterIndex::build(
            source.clone(),
            vec![f.clone()],
            &Cancellation::default(),
            &progress,
        )?;
        assert_eq!(index.matches, 1001);
        assert_eq!(progress.load(Ordering::Acquire), 10000);
        let page = index.load(1000, RowLimit::Paged(500), 0, &Cancellation::default())?;
        assert_eq!(page.frame.height(), 1);
        assert_eq!(page.source_row(0), 9500);
        assert_eq!(page.inspect_cell(0, 0)?.source_row, 9501);
        assert_eq!(page.page_count(), 3);
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(FilterIndex::build(source, vec![f], &cancel, &progress).is_err());
        Ok(())
    }

    #[test]
    fn text_boolean_null_and_combined_filters_work_across_formats() -> Result<()> {
        let directory = tempfile::tempdir()?;
        for extension in ["parquet", "ipc", "csv"] {
            let path = directory.path().join(format!("data.{extension}"));
            let mut frame = df!("text"=>&[Some("North"), Some("north"), Some(""), None], "active"=>&[Some(true),Some(false),None,Some(true)])?;
            match extension {
                "parquet" => {
                    ParquetWriter::new(File::create(&path)?).finish(&mut frame)?;
                }
                "ipc" => {
                    IpcWriter::new(File::create(&path)?).finish(&mut frame)?;
                }
                _ => {
                    CsvWriter::new(File::create(&path)?).finish(&mut frame)?;
                }
            }
            let source = Source::open(
                crate::source::Location::local(path),
                Default::default(),
                "",
                &Cancellation::default(),
            )?;
            let make = |column: &str, op, value: &str| Filter {
                column: column.into(),
                op,
                value: value.into(),
                upper: String::new(),
            };
            let index = FilterIndex::build(
                source.clone(),
                vec![
                    make("text", FilterOp::Contains, "North"),
                    make("active", FilterOp::Equals, "true"),
                ],
                &Cancellation::default(),
                &AtomicUsize::new(0),
            )?;
            let page = index.load(0, RowLimit::All, 0, &Cancellation::default())?;
            assert_eq!(page.frame.height(), 1);
            assert_eq!(page.source_row(0), 0);
            let index = FilterIndex::build(
                source.clone(),
                vec![make("active", FilterOp::IsNull, "")],
                &Cancellation::default(),
                &AtomicUsize::new(0),
            )?;
            assert_eq!(index.matches, 1);
            let page = index.load(0, RowLimit::Paged(100), 0, &Cancellation::default())?;
            assert_eq!(page.source_row(0), 2);
            let index = FilterIndex::build(
                source.clone(),
                vec![make("text", FilterOp::Equals, "absent")],
                &Cancellation::default(),
                &AtomicUsize::new(0),
            )?;
            let page = index.load(0, RowLimit::All, 0, &Cancellation::default())?;
            assert_eq!(page.frame.height(), 0);
            assert_eq!(page.frame.width(), 2);
            assert_eq!(page.page_count(), 1);
            if extension != "csv" {
                let index = FilterIndex::build(
                    source,
                    vec![make("text", FilterOp::Equals, "")],
                    &Cancellation::default(),
                    &AtomicUsize::new(0),
                )?;
                assert_eq!(index.matches, 1);
            }
        }
        Ok(())
    }

    #[test]
    fn rejects_reversed_ranges_and_unrepresentable_bounds() -> Result<()> {
        let frame = df!("n"=>&[1i64,2,3])?;
        assert!(
            validate(
                &[Filter {
                    column: "n".into(),
                    op: FilterOp::Range,
                    value: "3".into(),
                    upper: "1".into()
                }],
                &frame
            )
            .is_err()
        );
        assert!(bound("1.234", &DataType::Decimal(10, 2)).is_err());
        assert_eq!(
            bound("1.23", &DataType::Decimal(10, 2))?,
            AnyValue::Decimal(123, 10, 2)
        );
        assert!(
            bound(
                "2026-10-05T00:00:00.000001Z",
                &DataType::Datetime(TimeUnit::Milliseconds, None)
            )
            .is_err()
        );
        let timestamp = bound(
            "2026-10-05T01:00:00+01:00",
            &DataType::Datetime(TimeUnit::Microseconds, None),
        )?;
        assert_eq!(
            timestamp,
            bound(
                "2026-10-05T00:00:00Z",
                &DataType::Datetime(TimeUnit::Microseconds, None)
            )?
        );
        Ok(())
    }
}
