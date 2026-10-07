use anyhow::Result;
use parquetviewer::{
    data::{self, RowLimit},
    filter::{Filter, FilterIndex, FilterOp},
    query,
    source::{Cancellation, CsvOptions, Location, Source},
};
use polars_core::prelude::*;
use polars_io::prelude::*;
use std::{
    fs::File,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn write(path: &Path, start: i64, rows: usize) -> Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut frame = df!["id" => (start..start + rows as i64).collect::<Vec<_>>(), "flag" => (0..rows).map(|i| i % 2 == 0).collect::<Vec<_>>(), "note" => (0..rows).map(|i| if i % 3 == 0 { None } else { Some("été") }).collect::<Vec<_>>()]?;
    ParquetWriter::new(File::create(path)?)
        .with_row_group_size(Some(100))
        .finish(&mut frame)?;
    Ok(())
}
fn open(path: &Path) -> Result<Arc<Source>> {
    Source::open(
        Location::local(path.to_owned()),
        CsvOptions::default(),
        "",
        &Cancellation::default(),
    )
}
fn run(source: Arc<Source>, sql: &str) -> Result<Arc<query::QueryResult>> {
    query::run_with_executable(
        source,
        sql.into(),
        "".into(),
        &Cancellation::default(),
        &AtomicUsize::new(0),
        Path::new(env!("CARGO_BIN_EXE_parquetviewer")),
        1 << 30,
    )
}
fn hive(root: &Path) -> Result<()> {
    write(
        &root.join("year=2025/region=New%20York/part-1.parquet"),
        0,
        150,
    )?;
    write(
        &root.join("year=2026/region=__HIVE_DEFAULT_PARTITION__/part-2.parquet"),
        150,
        200,
    )?;
    std::fs::write(root.join("_SUCCESS"), "")?;
    std::fs::write(root.join("notes.txt"), "ignored")?;
    Ok(())
}

#[test]
fn hive_pages_cross_partitions_and_filters_include_virtual_columns() -> Result<()> {
    let directory = tempfile::tempdir()?;
    hive(directory.path())?;
    let source = open(directory.path())?;
    assert!(source.is_dataset());
    assert_eq!(source.info.total_rows, 350);
    assert_eq!(source.info.files, 2);
    let page = data::load_source(
        source.clone(),
        100,
        RowLimit::Paged(100),
        0,
        &Cancellation::default(),
    )?;
    assert_eq!(page.frame.width(), 5);
    assert_eq!(page.frame.column("year")?.dtype(), &DataType::Int64);
    assert_eq!(page.frame.column("year")?.get(49)?, AnyValue::Int64(2025));
    assert_eq!(page.frame.column("year")?.get(50)?, AnyValue::Int64(2026));
    assert_eq!(
        page.frame.column("region")?.get(0)?,
        AnyValue::String("New York")
    );
    assert_eq!(page.frame.column("region")?.null_count(), 50);
    let index = FilterIndex::build(
        source.clone(),
        vec![Filter {
            column: "year".into(),
            op: FilterOp::Equals,
            value: "2026".into(),
            upper: "".into(),
        }],
        &Cancellation::default(),
        &AtomicUsize::new(0),
    )?;
    let filtered = index.load(100, RowLimit::Paged(100), 0, &Cancellation::default())?;
    assert_eq!(filtered.matching_rows, Some(200));
    assert_eq!(filtered.source_row(0), 250);
    assert_eq!(
        source
            .read_slice(350, 100, &Cancellation::default())?
            .height(),
        0
    );
    write(
        &directory
            .path()
            .join("year=2026/region=Elsewhere/part-3.parquet"),
        350,
        1,
    )?;
    assert!(source.check_snapshot(&Cancellation::default()).is_err());
    Ok(())
}

#[test]
fn hive_rejects_mixed_schemas_keys_and_cancelled_discovery() -> Result<()> {
    let directory = tempfile::tempdir()?;
    write(&directory.path().join("year=2025/a.parquet"), 0, 1)?;
    write(&directory.path().join("month=01/b.parquet"), 1, 1)?;
    assert!(
        open(directory.path())
            .err()
            .unwrap()
            .to_string()
            .contains("Partition keys")
    );
    std::fs::remove_dir_all(directory.path().join("month=01"))?;
    let mut different = df!["other" => [1i64]]?;
    ParquetWriter::new(File::create(directory.path().join("year=2025/b.parquet"))?)
        .finish(&mut different)?;
    assert!(
        open(directory.path())
            .err()
            .unwrap()
            .to_string()
            .contains("schemas differ")
    );
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(
        Source::open(
            Location::local(directory.path().to_owned()),
            CsvOptions::default(),
            "",
            &cancel
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn sql_queries_the_source_and_pages_ordered_transformed_results() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("données with spaces %.parquet");
    write(&path, 0, 1200)?;
    let source = open(&path)?;
    let result = run(
        source.clone(),
        "SELECT id, id * 2 AS doubled, note FROM data WHERE id >= 150 ORDER BY id DESC",
    )?;
    let first = result.load(0, RowLimit::Paged(100), 0, &Cancellation::default())?;
    assert_eq!(first.total_rows, 1050);
    assert_eq!(first.frame.column("id")?.get(0)?, AnyValue::Int64(1199));
    assert_eq!(
        first.frame.column("doubled")?.get(0)?,
        AnyValue::Int64(2398)
    );
    assert_eq!(first.source.location, source.location);
    let last = result.load(1000, RowLimit::Paged(100), 0, &Cancellation::default())?;
    assert_eq!(last.frame.height(), 50);
    assert_eq!(last.frame.column("id")?.get(49)?, AnyValue::Int64(150));
    let grouped = run(
        source.clone(),
        "SELECT flag, COUNT(*) AS n FROM data GROUP BY flag ORDER BY flag",
    )?;
    assert_eq!(
        grouped
            .load(0, RowLimit::Paged(100), 0, &Cancellation::default())?
            .frame
            .height(),
        2
    );
    let empty = run(
        source.clone(),
        "WITH selected AS (SELECT id FROM data WHERE id > 1500) SELECT * FROM selected",
    )?;
    let page = empty.load(0, RowLimit::Paged(100), 0, &Cancellation::default())?;
    assert_eq!(page.total_rows, 0);
    assert_eq!(page.profiles[0].name, "id");
    for sql in [
        "DROP TABLE data",
        "SELECT * FROM data; SELECT * FROM data",
        "SELECT * INTO other FROM data",
        "SELECT * FROM read_csv('private.csv')",
    ] {
        assert!(query::validate(sql).is_err(), "{sql}");
    }
    assert!(run(source, "SELECT absent FROM data").is_err());
    Ok(())
}

#[test]
fn sql_hive_partition_predicates_and_csv_options_use_the_whole_source() -> Result<()> {
    let directory = tempfile::tempdir()?;
    hive(directory.path())?;
    let result = run(
        open(directory.path())?,
        "SELECT year, COUNT(*) AS n, SUM(id) AS total FROM data WHERE region IS NULL GROUP BY year",
    )?;
    let page = result.load(0, RowLimit::Paged(100), 0, &Cancellation::default())?;
    assert_eq!(page.frame.height(), 1);
    assert_eq!(page.frame.column("year")?.get(0)?, AnyValue::Int64(2026));
    assert_eq!(page.frame.column("total")?.get(0)?, AnyValue::Int64(49900));
    let csv = directory.path().join("text.csv");
    std::fs::write(&csv, "id;note\n001;été\n002;\"two\nlines\"\n")?;
    let source = Source::open(
        Location::local(csv),
        CsvOptions {
            delimiter: parquetviewer::source::Delimiter::Semicolon,
            infer_types: false,
            ..Default::default()
        },
        "",
        &Cancellation::default(),
    )?;
    let result = run(source, "SELECT id, note FROM data WHERE id = '002'")?;
    let page = result.load(0, RowLimit::Paged(100), 0, &Cancellation::default())?;
    assert_eq!(page.frame.column("id")?.get(0)?, AnyValue::String("002"));
    assert_eq!(
        page.frame.column("note")?.get(0)?,
        AnyValue::String("two\nlines")
    );
    Ok(())
}

#[test]
fn sql_worker_memory_limit_and_cancellation_leave_source_usable() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("input.parquet");
    write(&path, 0, 5000)?;
    let source = open(&path)?;
    let error = query::run_with_executable(
        source.clone(),
        query::DEFAULT_QUERY.into(),
        "".into(),
        &Cancellation::default(),
        &AtomicUsize::new(0),
        Path::new(env!("CARGO_BIN_EXE_parquetviewer")),
        1,
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("memory budget"), "{error:#}");
    let cancel = Cancellation::default();
    let stop = cancel.clone();
    let progress = Arc::new(AtomicUsize::new(0));
    let watch = progress.clone();
    let thread = std::thread::spawn(move || {
        for _ in 0..20 {
            if watch.load(Ordering::Acquire) > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        stop.cancel();
    });
    let result = query::run_with_executable(
        source.clone(),
        "SELECT a.id FROM data a CROSS JOIN data b".into(),
        "".into(),
        &cancel,
        &progress,
        Path::new(env!("CARGO_BIN_EXE_parquetviewer")),
        1 << 30,
    );
    thread.join().unwrap();
    assert!(result.err().unwrap().to_string().contains("Cancelled"));
    assert_eq!(
        source
            .read_slice(0, 100, &Cancellation::default())?
            .height(),
        100
    );
    Ok(())
}

#[test]
fn hive_partition_inference_preserves_large_integer_ids_and_boolean_values() -> Result<()> {
    let directory = tempfile::tempdir()?;
    write(
        &directory
            .path()
            .join("key=18446744073709551615/active=true/data.parquet"),
        0,
        1,
    )?;
    write(
        &directory
            .path()
            .join("key=18446744073709551614/active=false/data.parquet"),
        1,
        1,
    )?;
    let source = open(directory.path())?;
    let frame = source.read_slice(0, 100, &Cancellation::default())?;
    assert_eq!(frame.column("key")?.dtype(), &DataType::String);
    assert_eq!(frame.column("active")?.dtype(), &DataType::Boolean);
    let page = run(source, "SELECT key, active FROM data ORDER BY key DESC")?.load(
        0,
        RowLimit::Paged(100),
        0,
        &Cancellation::default(),
    )?;
    assert_eq!(
        page.frame.column("key")?.get(0)?,
        AnyValue::String("18446744073709551615")
    );
    assert_eq!(
        page.frame.column("active")?.get(0)?,
        AnyValue::Boolean(true)
    );
    Ok(())
}

#[test]
fn sql_compressed_ipc_preserves_temporal_and_nested_values_across_batches() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("été % input.arrow");
    let mut frame = df!["id" => (0..4200u64).collect::<Vec<_>>(), "time" => (0..4200i64).collect::<Vec<_>>(), "text" => (0..4200).map(|i| if i % 2 == 0 {Some("été")} else {None}).collect::<Vec<_>>()]?;
    frame.with_column(
        frame
            .column("time")?
            .cast(&DataType::Datetime(TimeUnit::Microseconds, None))?,
    )?;
    let lists: Vec<Series> = (0..4200)
        .map(|i| Series::new("".into(), [i as i64, i as i64 + 1]))
        .collect();
    frame.with_column(Series::new("nested".into(), lists).into_column())?;
    IpcWriter::new(File::create(&path)?)
        .with_record_batch_size(Some(137))
        .with_compression(Some(IpcCompression::ZSTD(Default::default())))
        .finish(&mut frame)?;
    let result = run(
        open(&path)?,
        "SELECT id, time, text, nested FROM data WHERE id >= 100 ORDER BY id DESC",
    )?;
    let page = result.load(4000, RowLimit::Paged(100), 0, &Cancellation::default())?;
    assert_eq!(page.total_rows, 4100);
    assert_eq!(page.frame.column("id")?.get(99)?, AnyValue::UInt64(100));
    assert_eq!(
        page.frame.column("time")?.dtype(),
        &DataType::Datetime(TimeUnit::Microseconds, None)
    );
    let AnyValue::List(last) = page.frame.column("nested")?.get(99)? else {
        panic!("Expected a list");
    };
    assert_eq!(
        last.i64()?.into_no_null_iter().collect::<Vec<_>>(),
        vec![100, 101]
    );
    assert_eq!(page.frame.column("text")?.get(99)?, AnyValue::String("été"));
    assert_eq!(page.file_bytes, std::fs::metadata(path)?.len());
    Ok(())
}

#[cfg(feature = "cloud")]
#[test]
fn sql_signed_http_source_stages_all_rows_without_losing_credentials() -> Result<()> {
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        sync::atomic::AtomicBool,
    };
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("remote.parquet");
    write(&path, 0, 1200)?;
    let bytes = std::fs::read(path)?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let location = Location::remote(&format!(
        "http://{}/remote.parquet?token=test-secret",
        listener.local_addr()?
    ))?;
    let stop = Arc::new(AtomicBool::new(false));
    let changed = Arc::new(AtomicBool::new(false));
    struct Server {
        stop: Arc<AtomicBool>,
        worker: Option<std::thread::JoinHandle<()>>,
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.worker.take().unwrap().join().unwrap();
        }
    }
    let done = stop.clone();
    let version_changed = changed.clone();
    let worker = std::thread::spawn(move || {
        while !done.load(Ordering::Acquire) {
            let (mut stream, _) = match listener.accept() {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(_) => break,
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                continue;
            }
            let authenticated = line.contains("token=test-secret");
            let mut range = None;
            let mut complete = false;
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                if line == "\r\n" {
                    complete = true;
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                    let (start, end) = value.trim().split_once('-').unwrap();
                    range = Some((
                        start.parse::<usize>().unwrap(),
                        end.parse::<usize>().unwrap(),
                    ));
                }
            }
            if !complete {
                continue;
            }
            if !authenticated {
                let _ = write!(
                    stream,
                    "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                continue;
            }
            if let Some((start, end)) = range {
                let _ = write!(
                    stream,
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nETag: \"sql-test-{}\"\r\nConnection: close\r\n\r\n",
                    end - start + 1,
                    start,
                    end,
                    bytes.len(),
                    version_changed.load(Ordering::Acquire)
                );
                let _ = stream.write_all(&bytes[start..=end]);
            } else {
                let _ = write!(
                    stream,
                    "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
        }
    });
    let _server = Server {
        stop,
        worker: Some(worker),
    };
    let source = Source::open(
        location,
        CsvOptions::default(),
        "",
        &Cancellation::default(),
    )?;
    let page = run(
        source.clone(),
        "SELECT id FROM data WHERE id >= 1000 ORDER BY id DESC",
    )?
    .load(100, RowLimit::Paged(100), 0, &Cancellation::default())?;
    assert_eq!(page.total_rows, 200);
    assert_eq!(page.frame.column("id")?.get(99)?, AnyValue::Int64(1000));
    changed.store(true, Ordering::Release);
    let error = run(source, "SELECT COUNT(*) FROM data").err().unwrap();
    assert!(
        error.to_string().contains("Remote object changed"),
        "{error:#}"
    );
    Ok(())
}
