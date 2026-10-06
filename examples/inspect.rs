//! Read exactly one page without a GPU/window for diagnostics and measurements.
use anyhow::Result;
use parquetviewer::{
    data::{DEFAULT_PAGE_SIZE, RowLimit, load_source},
    source::{Cancellation, CsvOptions, Location, Source},
};
use std::{path::PathBuf, time::Instant};

fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let path = PathBuf::from(
        args.next()
            .ok_or_else(|| anyhow::anyhow!("Usage: inspect FILE [OFFSET|all]"))?,
    );
    let selection = args.next();
    let (offset, limit) = if selection.as_deref() == Some(std::ffi::OsStr::new("all")) {
        (0, RowLimit::All)
    } else {
        (
            selection
                .map(|v| v.to_string_lossy().parse::<usize>())
                .transpose()?
                .unwrap_or(0),
            RowLimit::Paged(DEFAULT_PAGE_SIZE),
        )
    };
    let started = Instant::now();
    let location = if let Some(value) = path.to_str().filter(|s| s.contains("://")) {
        Location::remote(value)?
    } else {
        Location::local(path)
    };
    let cancel = Cancellation::default();
    let source = Source::open(location, CsvOptions::default(), "", &cancel)?;
    let page = load_source(source, offset, limit, 0, &cancel)?;
    println!(
        "{} total rows; {} columns; page {} of {}; {} decoded rows; {:?}",
        page.total_rows,
        page.profiles.len(),
        page.page_number(),
        page.page_count(),
        page.frame.height(),
        started.elapsed()
    );
    for profile in &page.profiles {
        println!(
            "{}: {} ({} sampled nulls)",
            profile.name, profile.dtype, profile.null_count
        );
    }
    Ok(())
}
