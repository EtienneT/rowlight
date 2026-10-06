//! Statistics and clipboard formatting over bounded, already loaded data.
use anyhow::{Result, ensure};
use polars_core::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    Value,
    Null,
    Empty,
    Nan,
    Infinity,
}
pub fn cell_kind(value: &AnyValue<'_>) -> CellKind {
    match value {
        AnyValue::Null => CellKind::Null,
        AnyValue::String("") => CellKind::Empty,
        AnyValue::StringOwned(s) if s.is_empty() => CellKind::Empty,
        AnyValue::Float32(v) if v.is_nan() => CellKind::Nan,
        AnyValue::Float64(v) if v.is_nan() => CellKind::Nan,
        AnyValue::Float32(v) if v.is_infinite() => CellKind::Infinity,
        AnyValue::Float64(v) if v.is_infinite() => CellKind::Infinity,
        _ => CellKind::Value,
    }
}

fn number(v: f64) -> String {
    if !v.is_finite() {
        return "Outside floating-point range".into();
    }
    if v != 0.0 && !(0.0001..1e9).contains(&v.abs()) {
        return format!("{v:.6e}");
    }
    format!("{v:.6}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

pub fn statistics(column: &Column) -> Result<Vec<(String, String)>> {
    let mut stats = Vec::new();
    if column.dtype().is_numeric() {
        let numbers = column.cast(&DataType::Float64)?;
        let mut values: Vec<_> = numbers
            .f64()?
            .iter()
            .flatten()
            .filter(|v| v.is_finite())
            .collect();
        if values.is_empty() {
            return Ok(stats);
        }
        values.sort_by(f64::total_cmp);
        let scale = values
            .iter()
            .map(|v| v.abs())
            .fold(0.0_f64, f64::max)
            .max(1.0);
        let mean = values.iter().map(|v| v / scale).sum::<f64>() / values.len() as f64;
        let quantile = |p: f64| {
            let position = p * (values.len() - 1) as f64;
            let lower = position.floor() as usize;
            let upper = position.ceil() as usize;
            let t = position.fract();
            ((values[lower] / scale) * (1.0 - t) + (values[upper] / scale) * t) * scale
        };
        stats.extend([
            ("Mean".into(), number(mean * scale)),
            ("25th percentile".into(), number(quantile(0.25))),
            ("Median".into(), number(quantile(0.5))),
            ("75th percentile".into(), number(quantile(0.75))),
            (
                "Standard deviation (sample)".into(),
                if values.len() < 2 {
                    "Unavailable (one value)".into()
                } else {
                    number(
                        (values
                            .iter()
                            .map(|v| (v / scale - mean).powi(2))
                            .sum::<f64>()
                            / (values.len() - 1) as f64)
                            .sqrt()
                            * scale,
                    )
                },
            ),
        ]);
    } else if matches!(
        column.dtype(),
        DataType::String | DataType::Categorical(..) | DataType::Enum(..)
    ) {
        let strings = column.cast(&DataType::String)?;
        let mut count = 0usize;
        let mut sum = 0usize;
        let mut min = usize::MAX;
        let mut max = 0;
        let mut empty = 0;
        for s in strings.str()?.iter().flatten() {
            let len = s.chars().count();
            count += 1;
            sum += len;
            min = min.min(len);
            max = max.max(len);
            empty += usize::from(s.is_empty());
        }
        stats.push((
            "Empty strings".into(),
            format!(
                "{empty} ({:.1}% of non-null)",
                empty as f64 * 100.0 / count.max(1) as f64
            ),
        ));
        if count > 0 {
            stats.extend([
                ("Shortest (characters)".into(), min.to_string()),
                ("Longest (characters)".into(), max.to_string()),
                (
                    "Mean length (characters)".into(),
                    number(sum as f64 / count as f64),
                ),
            ]);
        }
    } else if matches!(column.dtype(), DataType::Boolean) {
        let values = column.bool()?;
        let non_null = values.len() - values.null_count();
        for target in [false, true] {
            let count = values.iter().flatten().filter(|v| *v == target).count();
            stats.push((
                target.to_string(),
                format!(
                    "{count} ({:.1}% of non-null)",
                    count as f64 * 100.0 / non_null.max(1) as f64
                ),
            ));
        }
    }
    Ok(stats)
}

/// TSV with quoted tabs/newlines/quotes, strict limits, and raw empty/null values.
/// Nulls use the explicit token `null`; empty strings remain empty fields.
pub fn copy_cells(
    frame: &DataFrame,
    rows: &[usize],
    columns: &[usize],
    include_header: bool,
) -> Result<String> {
    ensure!(
        rows.len().saturating_mul(columns.len()) <= 10_000,
        "Select at most 10,000 cells to copy"
    );
    let mut text = String::new();
    if include_header {
        write_record(
            &mut text,
            columns
                .iter()
                .map(|&c| Ok(frame.columns()[c].name().to_string())),
        )?;
    }
    for &row in rows {
        let fields = columns.iter().map(|&c| {
            let value = frame.columns()[c].get(row)?;
            let (text, truncated) = crate::data::format_value(&value, 65_536);
            ensure!(
                !truncated,
                "A cell exceeds the copy limit; use the cell inspector"
            );
            Ok(text)
        });
        write_record(&mut text, fields)?;
    }
    if !include_header && rows.len() == 1 && columns.len() == 1 {
        return Ok(crate::data::format_value(&frame.columns()[columns[0]].get(rows[0])?, 65_536).0);
    }
    text.pop();
    Ok(text)
}
fn write_record(text: &mut String, fields: impl Iterator<Item = Result<String>>) -> Result<()> {
    for (i, field) in fields.enumerate() {
        let field = field?;
        if i > 0 {
            text.push('\t');
        }
        if field.contains(['\t', '\n', '\r', '"']) {
            text.push('"');
            text.push_str(&field.replace('"', "\"\""));
            text.push('"');
        } else {
            text.push_str(&field);
        }
        ensure!(
            text.len() <= 1 << 20,
            "Copied selection exceeds 1 MiB; select fewer cells"
        );
    }
    text.push('\n');
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn summaries_exclude_missing_and_non_finite_and_use_unicode_lengths() -> Result<()> {
        let c = Column::new(
            "n".into(),
            [Some(1.0), Some(2.0), Some(3.0), Some(f64::NAN), None],
        );
        let stats = statistics(&c)?;
        assert!(stats.contains(&("Mean".into(), "2".into())));
        assert!(stats.contains(&("Median".into(), "2".into())));
        assert!(stats.contains(&("Standard deviation (sample)".into(), "1".into())));
        let stats = statistics(&Column::new("s".into(), [Some("é🦀"), Some(""), None]))?;
        assert!(stats.contains(&("Longest (characters)".into(), "2".into())));
        assert_eq!(cell_kind(&AnyValue::String("")), CellKind::Empty);
        assert_eq!(cell_kind(&AnyValue::Float64(f64::NAN)), CellKind::Nan);
        let huge = statistics(&Column::new("n".into(), [f64::MAX, -f64::MAX]))?;
        assert!(huge.contains(&("Mean".into(), "0".into())));
        Ok(())
    }
    #[test]
    fn copy_preserves_order_null_empty_and_quoted_fields() -> Result<()> {
        let frame = df!("a" => &[Some("a\tb"), Some(""), None], "b" => &[1,2,3])?;
        assert_eq!(
            copy_cells(&frame, &[2, 0, 1], &[1, 0], true)?,
            "b\ta\n3\tnull\n1\t\"a\tb\"\n2\t"
        );
        assert_eq!(copy_cells(&frame, &[0], &[0], false)?, "a\tb");
        assert!(copy_cells(&frame, &vec![0; 10001], &[0], false).is_err());
        let frame = df!("s" => vec!["x".repeat(60_000);20])?;
        assert!(copy_cells(&frame, &(0..20).collect::<Vec<_>>(), &[0], false).is_err());
        let frame = df!("s" => &["x".repeat(65_537)])?;
        assert!(copy_cells(&frame, &[0], &[0], false).is_err());
        Ok(())
    }
}
