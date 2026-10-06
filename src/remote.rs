//! Optional cloud transport. Only footer/column ranges, with bounded memory cache.

use crate::source::{Cancellation, MAX_FOOTER_BYTES};
use anyhow::{Context, Result, ensure};
use bytes::Bytes;
use object_store::{GetOptions, GetRange, ObjectMeta, ObjectStore, ObjectStoreExt};
use polars_core::prelude::*;
use polars_parquet::{
    parquet::{metadata::FileMetadata, read::deserialize_metadata},
    read::{BasicDecompressor, Filter, PageReader, column_iter_to_arrays, infer_schema},
};
use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    io::Cursor,
    ops::Range,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

const CACHE_BYTES: usize = 32 << 20;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
enum Transport {
    Http {
        client: reqwest::Client,
        url: url::Url,
    },
    Cloud {
        store: Arc<dyn ObjectStore>,
        path: object_store::path::Path,
    },
}
struct RangeReader {
    transport: Transport,
    size: u64,
    etag: Option<String>,
    version: Option<String>,
    last_modified: Option<String>,
    cache: Mutex<VecDeque<(Range<u64>, Bytes)>>,
}
pub struct RemoteParquet {
    reader: RangeReader,
    pub metadata: Arc<FileMetadata>,
    pub file_bytes: u64,
}

fn run<T>(cancel: &Cancellation, future: impl Future<Output = Result<T>>) -> Result<T> {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("network runtime")
    });
    cancel.check()?;
    runtime.block_on(async { tokio::select! {
        result = future => result,
        _ = cancel.cancelled() => anyhow::bail!("Cancelled"),
        _ = tokio::time::sleep(REQUEST_TIMEOUT) => anyhow::bail!("Remote request timed out after 30 seconds"),
    } })
}

impl RangeReader {
    fn open(value: &str, profile: &str, cancel: &Cancellation) -> Result<Self> {
        let url = url::Url::parse(value)?;
        if matches!(url.scheme(), "http" | "https") {
            let client = reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()?;
            // A one-byte GET works with signed URLs that authorize GET but not HEAD.
            let response = run(cancel, async {
                Ok(client
                    .get(url.clone())
                    .header(reqwest::header::RANGE, "bytes=0-0")
                    .send()
                    .await?)
            })
            .map_err(|_| {
                anyhow::anyhow!("Cannot reach remote object; check the URL, access and network")
            })?;
            ensure!(
                response.status() == reqwest::StatusCode::PARTIAL_CONTENT,
                "This server must support HTTP byte-range requests (206 Partial Content)"
            );
            let range = response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .context("Server did not return Content-Range")?;
            let size = range
                .strip_prefix("bytes 0-0/")
                .and_then(|v| v.parse::<u64>().ok())
                .context("Invalid remote object size")?;
            let etag = response
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|v| v.to_str().ok())
                .filter(|v| !v.starts_with("W/"))
                .map(str::to_owned);
            let last_modified = response
                .headers()
                .get(reqwest::header::LAST_MODIFIED)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            ensure!(
                etag.is_some() || last_modified.is_some(),
                "Server must provide ETag or Last-Modified to keep pages consistent"
            );
            return Ok(Self {
                transport: Transport::Http { client, url },
                size,
                etag,
                version: None,
                last_modified,
                cache: Mutex::default(),
            });
        }
        let mut options = if matches!(url.scheme(), "s3" | "s3a") {
            aws_profile_options(profile)?
        } else {
            Vec::new()
        };
        options.push(("timeout".into(), "30s".into()));
        let (store, path) = object_store::parse_url_opts(&url, options).map_err(|_| {
            anyhow::anyhow!("Cannot configure cloud source; check the provider URL and credentials")
        })?;
        let store: Arc<dyn ObjectStore> = store.into();
        let ObjectMeta {
            size,
            e_tag,
            version,
            ..
        } = run(cancel, async { Ok(store.head(&path).await?) }).map_err(|_| {
            anyhow::anyhow!(
                "Cannot access cloud object; check the URL, credential profile and permissions"
            )
        })?;
        ensure!(
            e_tag.is_some() || version.is_some(),
            "Cloud object needs an ETag or version identifier"
        );
        Ok(Self {
            transport: Transport::Cloud { store, path },
            size,
            etag: e_tag,
            version,
            last_modified: None,
            cache: Mutex::default(),
        })
    }
    fn range(&self, range: Range<u64>, cancel: &Cancellation) -> Result<Bytes> {
        cancel.check()?;
        ensure!(
            range.start < range.end && range.end <= self.size,
            "Invalid Parquet column range"
        );
        ensure!(
            range.end - range.start <= (crate::memory::all_rows_budget() / 4).min(256 << 20),
            "Remote column chunk exceeds the safe transfer budget. Use a file with smaller row groups"
        );
        if let Some(bytes) = self
            .cache
            .lock()
            .map_err(|_| anyhow::anyhow!("Range cache unavailable"))?
            .iter()
            .find_map(|(cached, bytes)| {
                (cached.start <= range.start && cached.end >= range.end).then(|| {
                    bytes.slice(
                        (range.start - cached.start) as usize..(range.end - cached.start) as usize,
                    )
                })
            })
        {
            return Ok(bytes);
        }
        let bytes = run(cancel, async {
            match &self.transport {
                Transport::Cloud { store, path } => {
                    let result = store.get_opts(path, GetOptions { range: Some(GetRange::Bounded(range.clone())), if_match: self.etag.clone(), version: self.version.clone(), ..Default::default() }).await?;
                    ensure!(result.range == range, "Cloud provider did not honor the requested range");
                    Ok(result.bytes().await?)
                }
                Transport::Http { client, url } => {
                    let mut request = client.get(url.clone()).header(reqwest::header::RANGE,format!("bytes={}-{}",range.start,range.end-1));
                    if let Some(etag) = &self.etag { request = request.header(reqwest::header::IF_MATCH,etag); }
                    else if let Some(modified) = &self.last_modified { request = request.header(reqwest::header::IF_UNMODIFIED_SINCE,modified); }
                    let mut response = request.send().await?;
                    ensure!(response.status()==reqwest::StatusCode::PARTIAL_CONTENT, "Remote object changed or the server stopped supporting ranges; reopen the source");
                    let expected = format!("bytes {}-{}/{}",range.start,range.end-1,self.size);
                    ensure!(response.headers().get(reqwest::header::CONTENT_RANGE).and_then(|v|v.to_str().ok())==Some(expected.as_str()), "Server returned the wrong byte range");
                    let length = (range.end-range.start) as usize;
                    let mut data = Vec::with_capacity(length);
                    while let Some(chunk) = response.chunk().await? { ensure!(data.len().saturating_add(chunk.len()) <= length,"Server exceeded the requested range"); data.extend_from_slice(&chunk); }
                    Ok(Bytes::from(data))
                }
            }
        }).map_err(|error| {
            // Avoid showing provider errors that can include a signed URL/query.
            let message = error.to_string();
            if message == "Cancelled" || message.starts_with("Remote request timed out") { error }
            else { anyhow::anyhow!("Remote range read failed; verify access, range support, or reopen a changed source") }
        })?;
        ensure!(
            bytes.len() as u64 == range.end - range.start,
            "Incomplete remote range"
        );
        if bytes.len() <= CACHE_BYTES {
            let mut cache = self
                .cache
                .lock()
                .map_err(|_| anyhow::anyhow!("Range cache unavailable"))?;
            while cache.iter().map(|(_, b)| b.len()).sum::<usize>() + bytes.len() > CACHE_BYTES {
                cache.pop_front();
            }
            cache.push_back((range, bytes.clone()));
        }
        Ok(bytes)
    }
}

impl RemoteParquet {
    pub fn open(url: &str, profile: &str, cancel: &Cancellation) -> Result<Self> {
        let reader = RangeReader::open(url, profile, cancel)?;
        ensure!(reader.size >= 12, "Invalid remote Parquet file");
        let trailer = reader.range(reader.size - 8..reader.size, cancel)?;
        ensure!(
            &trailer[4..] == b"PAR1",
            "Remote object is not a Parquet file"
        );
        let length = u32::from_le_bytes(trailer[..4].try_into()?) as u64;
        ensure!(
            length <= MAX_FOOTER_BYTES && length + 8 <= reader.size - 4,
            "Remote Parquet footer exceeds the safe size limit"
        );
        let footer = reader.range(reader.size - length - 8..reader.size, cancel)?;
        let metadata = Arc::new(deserialize_metadata(footer.to_vec().into())?);
        let file_bytes = reader.size;
        Ok(Self {
            reader,
            metadata,
            file_bytes,
        })
    }
    pub fn read_slice(
        &self,
        offset: usize,
        count: usize,
        cancel: &Cancellation,
    ) -> Result<DataFrame> {
        let schema = infer_schema(&self.metadata)?;
        let mut frame = crate::source::empty_arrow(&schema)?;
        let mut source_row = 0usize;
        let end = offset.saturating_add(count).min(self.metadata.num_rows);
        let budget = crate::memory::all_rows_budget();
        for group in &self.metadata.row_groups {
            cancel.check()?;
            let group_end = source_row.saturating_add(group.num_rows());
            if group_end <= offset {
                source_row = group_end;
                continue;
            }
            if source_row >= end {
                break;
            }
            let from = offset.saturating_sub(source_row);
            let to = end.min(group_end) - source_row;
            let mut columns = Vec::with_capacity(schema.len());
            for field in schema.iter_values() {
                cancel.check()?;
                let metas: Vec<_> = group
                    .columns_under_root_iter(field.name.as_str())
                    .context("Missing remote column")?
                    .collect();
                let estimate = metas.iter().fold(0u64, |sum, c| {
                    sum.saturating_add(c.uncompressed_size().max(0) as u64)
                        .saturating_add(c.compressed_size().max(0) as u64)
                });
                ensure!(estimate.saturating_mul(2).saturating_add(frame.estimated_size() as u64).saturating_add(columns.iter().map(|c: &Column| c.as_materialized_series().estimated_size() as u64).sum::<u64>()) <= budget,"Remote column exceeds the safe decoding budget. Use smaller row groups");
                let mut decoders = Vec::with_capacity(metas.len());
                let mut types = Vec::with_capacity(metas.len());
                for meta in metas {
                    let data = self.reader.range(meta.byte_range(), cancel)?;
                    decoders.push(BasicDecompressor::new(
                        PageReader::new(
                            Cursor::new(data.to_vec().into()),
                            meta,
                            vec![],
                            budget.min(256 << 20) as usize,
                        ),
                        vec![],
                    ));
                    types.push(&meta.descriptor().descriptor.primitive_type);
                }
                let (arrays, _) = column_iter_to_arrays(
                    decoders,
                    types,
                    field.clone(),
                    Some(Filter::new_ranged(from, to)),
                )?;
                columns.push(Series::try_from((field, arrays))?.into_column());
            }
            frame.vstack_mut(&DataFrame::new(to - from, columns)?)?;
            ensure!(
                frame.estimated_size() as u64 <= budget / 2,
                "Remote page exceeds the safe memory budget"
            );
            source_row = group_end;
        }
        cancel.check()?;
        Ok(frame)
    }
}

/// Standard shared credentials/config profiles; no secrets are stored by this app.
fn aws_profile_options(selected: &str) -> Result<Vec<(String, String)>> {
    if std::env::var_os("AWS_ACCESS_KEY_ID").is_some() {
        return Ok(Vec::new());
    }
    let profile = if selected.trim().is_empty() {
        std::env::var("AWS_PROFILE").unwrap_or_else(|_| "default".into())
    } else {
        selected.trim().into()
    };
    let Some(home) = dirs::home_dir() else {
        return Ok(Vec::new());
    };
    let paths = [
        std::env::var_os("AWS_CONFIG_FILE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| home.join(".aws/config")),
        std::env::var_os("AWS_SHARED_CREDENTIALS_FILE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| home.join(".aws/credentials")),
    ];
    let mut values = BTreeMap::new();
    for (index, path) in paths.iter().enumerate() {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => anyhow::bail!("Cannot read the AWS shared credential profile"),
        };
        let section = if index == 0 && profile != "default" {
            format!("profile {profile}")
        } else {
            profile.clone()
        };
        let mut current = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') && line.ends_with(']') {
                current = line[1..line.len() - 1].trim() == section;
            } else if current
                && !line.starts_with(['#', ';'])
                && let Some((key, value)) = line.split_once('=')
            {
                values.insert(key.trim().to_owned(), value.trim().to_owned());
            }
        }
    }
    if values
        .keys()
        .any(|key| key.starts_with("sso_") || key == "credential_process" || key == "role_arn")
        && !values.contains_key("aws_access_key_id")
    {
        anyhow::bail!(
            "This AWS profile uses SSO, role assumption or a credential process. Export temporary AWS credentials from your existing login, then reopen the source"
        );
    }
    Ok([
        ("aws_access_key_id", "access_key_id"),
        ("aws_secret_access_key", "secret_access_key"),
        ("aws_session_token", "token"),
        ("region", "region"),
    ]
    .into_iter()
    .filter_map(|(key, out)| values.get(key).map(|v| (out.into(), v.clone())))
    .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        data::{RowLimit, load_source},
        source::{CsvOptions, Location, Source},
    };
    use polars_io::prelude::*;
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        sync::atomic::{AtomicBool, Ordering},
        thread,
    };

    struct Server {
        url: String,
        stop: Arc<AtomicBool>,
        changed: Arc<AtomicBool>,
        ranges: Arc<Mutex<Vec<Range<u64>>>>,
        thread: Option<thread::JoinHandle<()>>,
    }
    impl Server {
        fn start(data: Vec<u8>, ranges_enabled: bool) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let url = format!(
                "http://{}/data.parquet?temporary=test-secret",
                listener.local_addr().unwrap()
            );
            let stop = Arc::new(AtomicBool::new(false));
            let changed = Arc::new(AtomicBool::new(false));
            let ranges = Arc::new(Mutex::new(Vec::new()));
            let (worker_stop, worker_changed, worker_ranges) =
                (stop.clone(), changed.clone(), ranges.clone());
            let worker = thread::spawn(move || {
                while !worker_stop.load(Ordering::Acquire) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(v) => v,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        Err(_) => break,
                    };
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut input = BufReader::new(stream.try_clone().unwrap());
                    let mut request = String::new();
                    let mut range = None;
                    let mut conditional = false;
                    let mut headers_complete = false;
                    loop {
                        request.clear();
                        if input.read_line(&mut request).unwrap_or(0) == 0 {
                            break;
                        }
                        if request == "\r\n" {
                            headers_complete = true;
                            break;
                        }
                        let lower = request.to_lowercase();
                        if let Some(value) = lower.strip_prefix("range: bytes=") {
                            let (start, end) = value.trim().split_once('-').unwrap();
                            range = Some(
                                start.parse::<u64>().unwrap()..end.parse::<u64>().unwrap() + 1,
                            );
                        }
                        if lower.starts_with("if-match:") {
                            conditional = true;
                        }
                    }
                    // Clients can open pooled connections and close them unused.
                    if !headers_complete {
                        continue;
                    }
                    if worker_changed.load(Ordering::Acquire) && conditional {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                        continue;
                    }
                    if !ranges_enabled {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            data.len()
                        );
                        continue;
                    }
                    let Some(range) = range else {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                        continue;
                    };
                    worker_ranges.lock().unwrap().push(range.clone());
                    let _ = write!(
                        stream,
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nETag: \"test-version-1\"\r\nConnection: close\r\n\r\n",
                        range.end - range.start,
                        range.start,
                        range.end - 1,
                        data.len()
                    );
                    let _ = stream.write_all(&data[range.start as usize..range.end as usize]);
                }
            });
            Self {
                url,
                stop,
                changed,
                ranges,
                thread: Some(worker),
            }
        }
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            self.thread.take().unwrap().join().unwrap();
        }
    }

    #[test]
    fn range_server_handles_abandoned_and_missing_range_requests() -> Result<()> {
        use std::net::{Shutdown, TcpStream};

        let server = Server::start(vec![0; 1024], true);
        let address = url::Url::parse(&server.url)?.socket_addrs(|| None)?[0];
        for request in ["", "GET /data.parquet HTTP/1.1\r\nRange: bytes=0-0"] {
            let mut stream = TcpStream::connect(address)?;
            stream.write_all(request.as_bytes())?;
            stream.shutdown(Shutdown::Write)?;
        }
        for (request, status) in [
            (
                "GET /data.parquet HTTP/1.1\r\nHost: localhost\r\n\r\n",
                "HTTP/1.1 400 Bad Request\r\n",
            ),
            (
                "GET /data.parquet HTTP/1.1\r\nHost: localhost\r\nRange: bytes=0-0\r\n\r\n",
                "HTTP/1.1 206 Partial Content\r\n",
            ),
        ] {
            let mut stream = TcpStream::connect(address)?;
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            stream.write_all(request.as_bytes())?;
            let mut response = String::new();
            BufReader::new(stream).read_line(&mut response)?;
            assert_eq!(response, status);
        }
        assert_eq!(*server.ranges.lock().unwrap(), vec![0..1]);
        Ok(())
    }

    #[test]
    fn remote_ranges_decode_typed_pages_cache_and_detect_changed_objects() -> Result<()> {
        let mut frame = df!("id"=>(0..12001u64).map(|v|u64::MAX-v).collect::<Vec<_>>(),"text"=>(0..12001).map(|v|format!("value-{v:08}-{}",v*713)).collect::<Vec<_>>(),"flag"=>(0..12001).map(|v|if v%3==0 {None}else{Some(v%2==0)}).collect::<Vec<_>>())?;
        let mut buffer = Cursor::new(Vec::new());
        ParquetWriter::new(&mut buffer)
            .with_row_group_size(Some(137))
            .finish(&mut frame)?;
        let file_size = buffer.get_ref().len() as u64;
        let server = Server::start(buffer.into_inner(), true);
        let cancel = Cancellation::default();
        let source = Source::open(
            Location::remote(&server.url)?,
            CsvOptions::default(),
            "",
            &cancel,
        )?;
        let page = load_source(Arc::clone(&source), 500, RowLimit::Paged(100), 0, &cancel)?;
        assert_eq!(page.total_rows, 12001);
        assert_eq!(page.frame.height(), 100);
        assert_eq!(page.cell(0, 0).0, (u64::MAX - 500).to_string());
        assert_eq!(page.cell(99, 1).0, "value-00000599-427087");
        assert_eq!(page.profiles[0].dtype, "u64");
        let requests = server.ranges.lock().unwrap().len();
        source.read_slice(500, 100, &cancel)?;
        assert_eq!(
            server.ranges.lock().unwrap().len(),
            requests,
            "same chunks should come from the bounded cache"
        );
        assert!(
            server
                .ranges
                .lock()
                .unwrap()
                .iter()
                .map(|r| r.end - r.start)
                .sum::<u64>()
                < file_size,
            "opening a page must not transfer the whole object"
        );
        server.changed.store(true, Ordering::Release);
        let error = source
            .read_slice(1000, 100, &cancel)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("test-secret"));
        assert!(error.contains("Remote range read failed"));
        Ok(())
    }
    #[test]
    fn remote_filters_scan_and_page_without_losing_source_positions() -> Result<()> {
        let mut frame = df!("id"=> (0..5001u64).collect::<Vec<_>>())?;
        let mut buffer = Cursor::new(Vec::new());
        ParquetWriter::new(&mut buffer)
            .with_row_group_size(Some(137))
            .finish(&mut frame)?;
        let server = Server::start(buffer.into_inner(), true);
        let cancel = Cancellation::default();
        let source = Source::open(
            Location::remote(&server.url)?,
            Default::default(),
            "",
            &cancel,
        )?;
        let index = crate::filter::FilterIndex::build(
            source,
            vec![crate::filter::Filter {
                column: "id".into(),
                op: crate::filter::FilterOp::Range,
                value: "4000".into(),
                upper: "5000".into(),
            }],
            &cancel,
            &std::sync::atomic::AtomicUsize::new(0),
        )?;
        assert_eq!(index.matches, 1001);
        let page = index.load(1000, RowLimit::Paged(500), 0, &cancel)?;
        assert_eq!(page.frame.height(), 1);
        assert_eq!(page.source_row(0), 5000);
        assert_eq!(page.cell(0, 0).0, "5000");
        Ok(())
    }
    #[test]
    fn ignores_no_range_servers_and_cancels_pending_requests() -> Result<()> {
        let server = Server::start(vec![0; 1024], false);
        let cancel = Cancellation::default();
        assert!(RemoteParquet::open(&server.url, "", &cancel).is_err());
        let interrupt = cancel.clone();
        let thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            interrupt.cancel();
        });
        let started = std::time::Instant::now();
        assert!(
            run::<()>(&cancel, std::future::pending())
                .unwrap_err()
                .to_string()
                .contains("Cancelled")
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        thread.join().unwrap();
        Ok(())
    }
}
