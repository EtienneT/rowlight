//! Generate a repeatable demo or large paging fixture in bounded batches.
use anyhow::{Result, ensure};
use polars_core::prelude::*;
use polars_io::prelude::*;
use std::{fs::File, path::PathBuf};

fn batch(start: usize, len: usize) -> PolarsResult<DataFrame> {
    let rows = start..start + len;
    let id: Vec<_> = rows.clone().map(|i| i as u64 + 1).collect();
    let amount: Vec<_> = rows
        .clone()
        .map(|i| {
            if i % 37 == 0 {
                None
            } else {
                Some(((i as f64 * 0.173).sin() * 35.0 + 85.0) * 100.0).map(|v| v.round() / 100.0)
            }
        })
        .collect();
    let active: Vec<_> = rows
        .clone()
        .map(|i| if i % 23 == 0 { None } else { Some(i % 5 != 0) })
        .collect();
    let regions = ["North", "South", "East", "West", "Central"];
    let region: Vec<_> = rows
        .clone()
        .map(|i| regions[(i * 7 % 13) % regions.len()])
        .collect();
    let dates: Vec<_> = rows.clone().map(|i| 20_000 + (i % 365) as i32).collect();
    let date = Series::new("created_at".into(), dates).cast(&DataType::Date)?;
    let note: Vec<_> = rows
        .map(|i| {
            if i % 7 == 0 {
                Some("Review requested")
            } else {
                None
            }
        })
        .collect();
    let mut frame = df!("id" => id, "amount" => amount, "active" => active, "region" => region, "note" => note)?;
    frame.with_column(date.into_column())?;
    Ok(frame)
}

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let path = PathBuf::from(args.next().unwrap_or_else(|| "samples/demo.parquet".into()));
    let count = args
        .next()
        .map(|s| s.to_string_lossy().parse::<usize>())
        .transpose()?
        .unwrap_or(50_000);
    ensure!(count > 0, "Row count must be positive");
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let first = batch(0, count.min(25_000))?;
    // Refuse to overwrite the user's data, including on accidental reruns.
    let file = File::options().write(true).create_new(true).open(&path)?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    match extension {
        "ipc" | "arrow" | "feather" => {
            let schema = first.schema().to_arrow(CompatLevel::newest());
            let fields = polars_arrow::io::ipc::write::default_ipc_fields(schema.iter_values());
            let mut writer = IpcWriter::new(file).batched(first.schema(), fields)?;
            writer.write_batch(&first)?;
            for start in (25_000..count).step_by(25_000) {
                writer.write_batch(&batch(start, (count - start).min(25_000))?)?;
            }
            writer.finish()?;
        }
        "csv" | "tsv" => {
            let mut file = file;
            let separator = if extension == "tsv" { b'\t' } else { b',' };
            for start in (0..count).step_by(25_000) {
                let mut frame = batch(start, (count - start).min(25_000))?;
                CsvWriter::new(&mut file)
                    .include_header(start == 0)
                    .with_separator(separator)
                    .finish(&mut frame)?;
            }
        }
        _ => {
            let mut writer = ParquetWriter::new(file).batched(first.schema())?;
            writer.write_batch(&first)?;
            for start in (25_000..count).step_by(25_000) {
                writer.write_batch(&batch(start, (count - start).min(25_000))?)?;
            }
            writer.finish()?;
        }
    }
    println!("Created {} with {count} rows", path.display());
    Ok(())
}
