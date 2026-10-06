//! One source, bounded slices. CSV indexes live on disk, not in a giant dataframe.

use std::{
    collections::BTreeSet,
    fs::File,
    io::{BufRead, BufReader, Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::SystemTime,
};

use anyhow::{Context, Result, ensure};
use polars_arrow::io::ipc::read as ipc;
use polars_core::prelude::*;
use polars_io::prelude::*;
use polars_parquet::parquet::metadata::FileMetadata;
use serde::{Deserialize, Serialize};

const INDEX_STRIDE: usize = 100;
pub const MAX_RECORD_BYTES: u64 = 16 << 20;
pub const MAX_FOOTER_BYTES: u64 = 64 << 20;

#[derive(Clone, Default)]
pub struct Cancellation(Arc<CancelState>);
#[derive(Default)]
struct CancelState {
    cancelled: AtomicBool,
    #[cfg(feature = "cloud")]
    notify: tokio::sync::Notify,
}
impl Cancellation {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        #[cfg(feature = "cloud")]
        self.0.notify.notify_one();
    }
    pub fn check(&self) -> Result<()> {
        ensure!(!self.0.cancelled.load(Ordering::Acquire), "Cancelled");
        Ok(())
    }
    #[cfg(feature = "cloud")]
    pub(crate) async fn cancelled(&self) {
        self.0.notify.notified().await;
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Location {
    Local(PathBuf),
    Remote(String),
}
impl Location {
    pub fn local(path: PathBuf) -> Self {
        Self::Local(std::fs::canonicalize(&path).unwrap_or(path))
    }
    pub fn remote(value: &str) -> Result<Self> {
        let mut url = url::Url::parse(value.trim()).context("Enter a complete source URL")?;
        ensure!(
            matches!(
                url.scheme(),
                "s3" | "s3a" | "gs" | "gcs" | "az" | "azure" | "abfs" | "abfss" | "http" | "https"
            ),
            "Use an S3, Azure, GCS, or HTTP(S) URL"
        );
        ensure!(
            url.host_str().is_some() && !url.path().trim_matches('/').is_empty(),
            "Choose one remote Parquet object"
        );
        ensure!(
            (url.username().is_empty() || matches!(url.scheme(), "abfs" | "abfss"))
                && url.password().is_none(),
            "Use credential profiles or signed URLs instead of embedding a username/password"
        );
        match url.scheme() {
            "s3a" => {
                let _ = url.set_scheme("s3");
            }
            "gcs" => {
                let _ = url.set_scheme("gs");
            }
            "azure" => {
                let _ = url.set_scheme("az");
            }
            _ => {}
        }
        Ok(Self::Remote(url.to_string()))
    }
    pub fn display(&self) -> String {
        match self {
            Self::Local(path) => path.display().to_string(),
            Self::Remote(value) => {
                let mut url = url::Url::parse(value).expect("validated source URL");
                url.set_query(None);
                url.set_fragment(None);
                url.to_string()
            }
        }
    }
    pub fn name(&self) -> String {
        match self {
            Self::Local(path) => path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            Self::Remote(value) => url::Url::parse(value)
                .ok()
                .and_then(|u| {
                    u.path_segments()
                        .and_then(|mut p| p.next_back().map(str::to_owned))
                })
                .unwrap_or_else(|| "Remote Parquet".into()),
        }
    }
    /// Signed URLs may contain secrets and must never enter history/preferences.
    pub fn rememberable(&self) -> bool {
        match self {
            Self::Local(path) => path.to_str().is_some(),
            Self::Remote(value) => {
                url::Url::parse(value).is_ok_and(|u| u.query().is_none() && u.fragment().is_none())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delimiter {
    #[default]
    Auto,
    Comma,
    Tab,
    Semicolon,
    Pipe,
}
impl Delimiter {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automatic",
            Self::Comma => "Comma",
            Self::Tab => "Tab",
            Self::Semicolon => "Semicolon",
            Self::Pipe => "Pipe",
        }
    }
    pub fn byte(self, path: &Path) -> u8 {
        match self {
            Self::Auto
                if path
                    .extension()
                    .is_some_and(|v| v.eq_ignore_ascii_case("tsv")) =>
            {
                b'\t'
            }
            Self::Tab => b'\t',
            Self::Semicolon => b';',
            Self::Pipe => b'|',
            _ => b',',
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CsvOptions {
    pub delimiter: Delimiter,
    pub has_header: bool,
    pub lossy_utf8: bool,
    pub infer_types: bool,
}
impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: Delimiter::Auto,
            has_header: true,
            lossy_utf8: false,
            infer_types: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GroupInfo {
    pub rows: usize,
    pub compressed: u64,
    pub uncompressed: u64,
}
#[derive(Clone, Debug)]
pub struct SourceInfo {
    pub format: &'static str,
    pub total_rows: usize,
    pub file_bytes: u64,
    pub groups: Vec<GroupInfo>,
    pub codecs: Vec<String>,
    pub encodings: Vec<String>,
    pub created_by: Option<String>,
    pub inferred_types: bool,
}

enum Kind {
    Parquet(Arc<FileMetadata>),
    Ipc(ipc::FileMetadata, Vec<usize>),
    Csv(CsvIndex),
    #[cfg(feature = "cloud")]
    Remote(crate::remote::RemoteParquet),
}
pub struct Source {
    pub location: Location,
    pub info: SourceInfo,
    pub csv_options: CsvOptions,
    stamp: Option<(u64, Option<SystemTime>)>,
    kind: Kind,
}
impl Source {
    pub fn open(
        location: Location,
        csv_options: CsvOptions,
        profile: &str,
        cancel: &Cancellation,
    ) -> Result<Arc<Self>> {
        cancel.check()?;
        if let Location::Remote(url) = &location {
            #[cfg(feature = "cloud")]
            {
                let remote = crate::remote::RemoteParquet::open(url, profile, cancel)?;
                let info = parquet_info(&remote.metadata, remote.file_bytes);
                return Ok(Arc::new(Self {
                    location,
                    info,
                    csv_options,
                    stamp: None,
                    kind: Kind::Remote(remote),
                }));
            }
            #[cfg(not(feature = "cloud"))]
            {
                let _ = (url, profile);
                anyhow::bail!("Remote sources require the cloud build of Parquet Viewer");
            }
        }
        let Location::Local(path) = &location else {
            unreachable!()
        };
        let mut file =
            File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
        let stat = file.metadata()?;
        ensure!(stat.is_file(), "Choose a data file");
        let stamp = Some((stat.len(), stat.modified().ok()));
        let mut magic = [0u8; 6];
        let n = file.read(&mut magic)?;
        file.rewind()?;
        let (kind, info) = if n >= 4 && &magic[..4] == b"PAR1" {
            check_footer(&mut file, stat.len(), false)?;
            let metadata = Arc::new(polars_parquet::parquet::read::read_metadata(&mut file)?);
            let info = parquet_info(&metadata, stat.len());
            (Kind::Parquet(metadata), info)
        } else if n == 6 && &magic == b"ARROW1" {
            check_footer(&mut file, stat.len(), true)?;
            let metadata = ipc::read_file_metadata(&mut file)?;
            let mut ends = Vec::with_capacity(metadata.blocks.len());
            let mut total = 0usize;
            for block in &metadata.blocks {
                cancel.check()?;
                ensure!(
                    block.meta_data_length >= 0
                        && block.meta_data_length as u64 <= MAX_FOOTER_BYTES,
                    "IPC batch metadata is too large"
                );
                let rows = ipc::get_row_count_from_blocks(&mut file, std::slice::from_ref(block))?;
                total = total
                    .checked_add(usize::try_from(rows)?)
                    .context("IPC row count is too large")?;
                ends.push(total);
            }
            let info = SourceInfo {
                format: "Arrow IPC / Feather v2",
                total_rows: total,
                file_bytes: stat.len(),
                groups: metadata
                    .blocks
                    .iter()
                    .zip(&ends)
                    .enumerate()
                    .map(|(ix, (block, end))| GroupInfo {
                        rows: end - if ix == 0 { 0 } else { ends[ix - 1] },
                        compressed: block.body_length.max(0) as u64,
                        uncompressed: 0,
                    })
                    .collect(),
                codecs: vec!["IPC batch compression (if present)".into()],
                encodings: Vec::new(),
                created_by: None,
                inferred_types: false,
            };
            (Kind::Ipc(metadata, ends), info)
        } else {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            ensure!(
                ext.eq_ignore_ascii_case("csv") || ext.eq_ignore_ascii_case("tsv"),
                "Unsupported format. Open Parquet, Arrow IPC/Feather v2, CSV, or TSV"
            );
            let index = CsvIndex::build(path, csv_options, cancel)?;
            let info = SourceInfo {
                format: if ext.eq_ignore_ascii_case("tsv") {
                    "TSV"
                } else {
                    "CSV"
                },
                total_rows: index.rows,
                file_bytes: stat.len(),
                groups: Vec::new(),
                codecs: Vec::new(),
                encodings: vec![
                    if csv_options.lossy_utf8 {
                        "UTF-8 with replacement"
                    } else {
                        "UTF-8"
                    }
                    .into(),
                ],
                created_by: None,
                inferred_types: csv_options.infer_types,
            };
            (Kind::Csv(index), info)
        };
        cancel.check()?;
        let source = Arc::new(Self {
            location,
            info,
            csv_options,
            stamp,
            kind,
        });
        source.check_unchanged()?;
        Ok(source)
    }

    pub fn check_unchanged(&self) -> Result<()> {
        if let (Location::Local(path), Some(stamp)) = (&self.location, self.stamp) {
            let stat = std::fs::metadata(path)?;
            ensure!(
                (stat.len(), stat.modified().ok()) == stamp,
                "The file changed. Reopen it to refresh the schema and row index"
            );
        }
        Ok(())
    }

    pub fn read_slice(
        &self,
        offset: usize,
        count: usize,
        cancel: &Cancellation,
    ) -> Result<DataFrame> {
        self.check_unchanged()?;
        cancel.check()?;
        let budget = crate::memory::all_rows_budget();
        let frame = match &self.kind {
            Kind::Parquet(metadata) => {
                let Location::Local(path) = &self.location else {
                    unreachable!()
                };
                let mut first_row = 0usize;
                for group in &metadata.row_groups {
                    let end = first_row.saturating_add(group.num_rows());
                    if end > offset && first_row < offset.saturating_add(count) {
                        let peak = group
                            .parquet_columns()
                            .iter()
                            .map(|c| c.uncompressed_size().max(0) as u64)
                            .max()
                            .unwrap_or(0);
                        ensure!(
                            peak.saturating_mul(2) <= budget,
                            "Parquet column chunk exceeds the safe decoding budget. Use a file with smaller row groups"
                        );
                    }
                    first_row = end;
                }
                let mut reader = ParquetReader::new(File::open(path)?)
                    .set_low_memory(true)
                    .read_parallel(ParallelStrategy::None)
                    .with_slice(Some((offset, count)));
                reader.set_metadata(Arc::clone(metadata));
                reader.finish()?
            }
            Kind::Ipc(metadata, ends) => {
                let Location::Local(path) = &self.location else {
                    unreachable!()
                };
                let start = ends.partition_point(|end| *end <= offset);
                let within = offset - if start == 0 { 0 } else { ends[start - 1] };
                let last = ends.partition_point(|end| *end < offset.saturating_add(count));
                for block in metadata
                    .blocks
                    .iter()
                    .take(last.saturating_add(1))
                    .skip(start)
                    .chain(metadata.dictionaries.iter().flatten())
                {
                    ensure!(
                        (block.body_length.max(0) as u64).saturating_mul(4) < budget,
                        "IPC batch exceeds the safe decoding budget. Use a file with smaller record batches"
                    );
                }
                let mut reader = ipc::FileReader::new(
                    File::open(path)?,
                    metadata.clone(),
                    None,
                    Some(within.saturating_add(count)),
                );
                reader.set_current_block(start);
                let mut result = empty_arrow(&metadata.schema)?;
                let mut skip = within;
                let mut remaining = count;
                for batch in reader {
                    cancel.check()?;
                    let batch = batch?;
                    let height = batch.len();
                    let columns = batch
                        .schema()
                        .iter_values()
                        .zip(batch.arrays())
                        .map(|(field, array)| {
                            Series::try_from((field, array.clone())).map(IntoColumn::into_column)
                        })
                        .collect::<PolarsResult<Vec<_>>>()?;
                    let chunk = DataFrame::new(height, columns)?.slice(skip as i64, remaining);
                    remaining -= chunk.height();
                    skip = 0;
                    result.vstack_mut(&chunk)?;
                    ensure!(
                        result.estimated_size() as u64 <= budget / 2,
                        "IPC page exceeds the safe memory budget"
                    );
                    if remaining == 0 {
                        break;
                    }
                }
                result
            }
            Kind::Csv(index) => {
                let Location::Local(path) = &self.location else {
                    unreachable!()
                };
                index.read(path, offset, count, self.csv_options, cancel)?
            }
            #[cfg(feature = "cloud")]
            Kind::Remote(remote) => remote.read_slice(offset, count, cancel)?,
        };
        self.check_unchanged()?;
        cancel.check()?;
        Ok(frame)
    }

    pub fn allocation_estimate(&self, columns: usize) -> u64 {
        let floor = (self.info.total_rows as u64)
            .saturating_mul((columns as u64).saturating_mul(16).saturating_add(64));
        let decoded = self
            .info
            .groups
            .iter()
            .fold(0u64, |sum, g| sum.saturating_add(g.uncompressed));
        floor
            .max(decoded.saturating_mul(8))
            .max(if self.info.groups.is_empty() {
                self.info.file_bytes.saturating_mul(8)
            } else {
                0
            })
            .saturating_add(32 << 20)
    }
}

fn check_footer(file: &mut File, size: u64, arrow: bool) -> Result<()> {
    let trailer = if arrow { 10 } else { 8 };
    ensure!(size >= trailer + 4, "Invalid data file");
    file.seek(SeekFrom::End(-(trailer as i64)))?;
    let mut length = [0u8; 4];
    file.read_exact(&mut length)?;
    ensure!(
        u32::from_le_bytes(length) as u64 <= MAX_FOOTER_BYTES,
        "File metadata exceeds the safe 64 MiB limit"
    );
    file.rewind()?;
    Ok(())
}

pub(crate) fn parquet_info(metadata: &FileMetadata, file_bytes: u64) -> SourceInfo {
    let codecs: BTreeSet<_> = metadata
        .row_groups
        .iter()
        .flat_map(|g| g.parquet_columns())
        .map(|c| format!("{:?}", c.compression()))
        .collect();
    // Encoding identifiers are in the original Thrift footer, not exposed by the
    // lightweight Polars metadata API. Decode only that bounded footer for details.
    let mut encodings = BTreeSet::new();
    let mut protocol = polars_parquet_format::thrift::protocol::TCompactInputProtocol::new(
        metadata.footer_buf.as_ref(),
        MAX_FOOTER_BYTES as usize,
    );
    if let Ok(footer) = polars_parquet_format::FileMetaData::read_from_in_protocol(&mut protocol) {
        for column in footer.row_groups.iter().flat_map(|g| &g.columns) {
            if let Some(meta) = &column.meta_data {
                encodings.extend(meta.encodings.iter().map(|e| match e.0 {
                    0 => "PLAIN".into(),
                    2 => "PLAIN_DICTIONARY".into(),
                    3 => "RLE".into(),
                    4 => "BIT_PACKED".into(),
                    5 => "DELTA_BINARY_PACKED".into(),
                    6 => "DELTA_LENGTH_BYTE_ARRAY".into(),
                    7 => "DELTA_BYTE_ARRAY".into(),
                    8 => "RLE_DICTIONARY".into(),
                    9 => "BYTE_STREAM_SPLIT".into(),
                    other => format!("Encoding {other}"),
                }));
            }
        }
    }
    SourceInfo {
        format: "Parquet",
        total_rows: metadata.num_rows,
        file_bytes,
        groups: metadata
            .row_groups
            .iter()
            .map(|g| GroupInfo {
                rows: g.num_rows(),
                compressed: g.compressed_size() as u64,
                uncompressed: g.total_byte_size() as u64,
            })
            .collect(),
        codecs: codecs.into_iter().collect(),
        encodings: encodings.into_iter().collect(),
        created_by: metadata.created_by.clone(),
        inferred_types: false,
    }
}

struct CsvIndex {
    checkpoints: tempfile::NamedTempFile,
    rows: usize,
    schema: SchemaRef,
    data_start: u64,
    end: u64,
}
impl CsvIndex {
    fn build(path: &Path, options: CsvOptions, cancel: &Cancellation) -> Result<Self> {
        let mut input = BufReader::with_capacity(64 << 10, File::open(path)?);
        let mut parser = csv_core::ReaderBuilder::new()
            .delimiter(options.delimiter.byte(path))
            .build();
        let mut output = [0u8; 8192];
        let mut fields = [0usize; 1024];
        let mut position = 0u64;
        let mut record_start = 0u64;
        let mut records = 0usize;
        let mut data_start = 0u64;
        let mut sample_end = 0u64;
        let mut checkpoints = tempfile::NamedTempFile::new()?;
        loop {
            cancel.check()?;
            let bytes = input.fill_buf()?;
            let (state, consumed, _, _) = parser.read_record(bytes, &mut output, &mut fields);
            position = position
                .checked_add(consumed as u64)
                .context("CSV is too large")?;
            input.consume(consumed);
            ensure!(
                position - record_start <= MAX_RECORD_BYTES,
                "A CSV record exceeds the safe 16 MiB limit"
            );
            if state == csv_core::ReadRecordResult::Record {
                if records == 0 && options.has_header {
                    data_start = position;
                } else {
                    let row = records - usize::from(options.has_header);
                    if row.is_multiple_of(INDEX_STRIDE) {
                        checkpoints.write_all(&record_start.to_le_bytes())?;
                    }
                }
                records = records
                    .checked_add(1)
                    .context("CSV row count is too large")?;
                if records <= 1000 + usize::from(options.has_header) {
                    sample_end = position;
                }
                record_start = position;
            } else if state == csv_core::ReadRecordResult::End {
                break;
            }
        }
        checkpoints.write_all(&position.to_le_bytes())?;
        checkpoints.flush()?;
        let sample_bytes = sample_end.min(position);
        ensure!(
            sample_bytes <= 64 << 20,
            "CSV schema sample exceeds 64 MiB. Reduce record sizes"
        );
        let mut sample = Vec::new();
        File::open(path)?
            .take(sample_bytes)
            .read_to_end(&mut sample)?;
        let sample = csv_records(&sample, options.delimiter.byte(path), 0, usize::MAX, cancel)?;
        let mut read_options =
            csv_reader_options(path, options).with_has_header(options.has_header);
        if !options.infer_types {
            read_options = read_options.with_infer_schema_length(Some(0));
        }
        let frame = anyhow::Context::context(
            read_options
                .into_reader_with_file_handle(Cursor::new(sample))
                .finish(),
            "Cannot infer the CSV schema. Check its delimiter, header and encoding options",
        )?;
        Ok(Self {
            checkpoints,
            rows: records.saturating_sub(usize::from(options.has_header)),
            schema: Arc::clone(frame.schema()),
            data_start,
            end: position,
        })
    }
    fn read(
        &self,
        path: &Path,
        offset: usize,
        count: usize,
        options: CsvOptions,
        cancel: &Cancellation,
    ) -> Result<DataFrame> {
        if self.rows == 0 || count == 0 || offset >= self.rows {
            return Ok(DataFrame::empty_with_schema(&self.schema));
        }
        let checkpoint = offset / INDEX_STRIDE;
        let mut index = self.checkpoints.reopen()?;
        index.seek(SeekFrom::Start(checkpoint as u64 * 8))?;
        let mut value = [0u8; 8];
        index.read_exact(&mut value)?;
        let start = u64::from_le_bytes(value).max(self.data_start);
        let end_row = offset.saturating_add(count).min(self.rows);
        let end_checkpoint = end_row.div_ceil(INDEX_STRIDE);
        let end = if end_checkpoint * INDEX_STRIDE >= self.rows {
            self.end
        } else {
            index.seek(SeekFrom::Start(end_checkpoint as u64 * 8))?;
            index.read_exact(&mut value)?;
            u64::from_le_bytes(value)
        };
        ensure!(
            end >= start && end - start <= (crate::memory::all_rows_budget() / 4).min(128 << 20),
            "CSV page exceeds the safe buffer budget. Choose a smaller page size"
        );
        let mut file = File::open(path)?;
        file.seek(SeekFrom::Start(start))?;
        let mut buffer = Vec::new();
        buffer.try_reserve((end - start) as usize)?;
        let mut chunk = [0u8; 65536];
        let mut remaining = end - start;
        while remaining > 0 {
            cancel.check()?;
            let n = file.read(&mut chunk[..remaining.min(65536) as usize])?;
            ensure!(n > 0, "CSV changed while reading");
            buffer.extend_from_slice(&chunk[..n]);
            remaining -= n as u64;
        }
        let buffer = csv_records(
            &buffer,
            options.delimiter.byte(path),
            offset % INDEX_STRIDE,
            count,
            cancel,
        )?;
        let frame = anyhow::Context::context(
            csv_reader_options(path, options)
                .with_schema(Some(Arc::clone(&self.schema)))
                .with_has_header(false)
                .with_n_rows(Some(count))
                .into_reader_with_file_handle(Cursor::new(buffer))
                .finish(),
            "Cannot parse this CSV page with the inferred types. Try reading all columns as text in CSV options",
        )?;
        Ok(frame.slice(0, count))
    }
}
/// csv-core skips bare blank lines and treats CRLF as one terminator. Normalize
/// the selected raw records before handing them to Polars, whose blank-line
/// behavior differs, so indexes and displayed source-row numbers stay consistent.
fn csv_records(
    input: &[u8],
    separator: u8,
    skip: usize,
    count: usize,
    cancel: &Cancellation,
) -> Result<Vec<u8>> {
    let mut parser = csv_core::ReaderBuilder::new().delimiter(separator).build();
    let mut scratch = [0u8; 8192];
    let mut fields = [0usize; 1024];
    let mut position = 0usize;
    let mut start = 0usize;
    let mut record = 0usize;
    let mut output = Vec::new();
    loop {
        cancel.check()?;
        let (state, consumed, _, _) =
            parser.read_record(&input[position..], &mut scratch, &mut fields);
        position += consumed;
        if state == csv_core::ReadRecordResult::Record {
            if record >= skip {
                let bytes = &input[start..position];
                let first = bytes
                    .iter()
                    .position(|byte| !matches!(byte, b'\r' | b'\n'))
                    .unwrap_or(bytes.len());
                output.extend_from_slice(&bytes[first..]);
                if output.last() != Some(&b'\n') {
                    output.push(b'\n');
                }
            }
            record += 1;
            start = position;
            if record.saturating_sub(skip) >= count {
                break;
            }
        } else if state == csv_core::ReadRecordResult::End {
            break;
        }
    }
    Ok(output)
}
fn csv_reader_options(path: &Path, options: CsvOptions) -> CsvReadOptions {
    CsvReadOptions::default()
        .with_n_threads(Some(1))
        .with_infer_schema_length(Some(1000))
        .map_parse_options(|parse| {
            parse
                .with_separator(options.delimiter.byte(path))
                .with_try_parse_dates(options.infer_types)
                .with_encoding(if options.lossy_utf8 {
                    CsvEncoding::LossyUtf8
                } else {
                    CsvEncoding::Utf8
                })
        })
}

pub(crate) fn empty_arrow(schema: &polars_arrow::datatypes::ArrowSchema) -> Result<DataFrame> {
    Ok(DataFrame::new(
        0,
        schema
            .iter_values()
            .map(|field| {
                Series::new_empty(
                    field.name.clone(),
                    &DataType::from_arrow(field.dtype(), field.metadata.as_deref()),
                )
                .into_column()
            })
            .collect(),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{RowLimit, load_source, overview};

    #[test]
    fn ipc_pages_cross_batches_preserve_types_and_empty_schema() -> Result<()> {
        for compression in [
            None,
            Some(IpcCompression::ZSTD(Default::default())),
            Some(IpcCompression::LZ4),
        ] {
            let file = tempfile::NamedTempFile::new()?;
            let mut frame = df!("id" => (0..2501u64).collect::<Vec<_>>(), "text" => (0..2501).map(|v| format!("value-{v}")).collect::<Vec<_>>(), "flag"=>vec![true;2501])?;
            IpcWriter::new(file.reopen()?)
                .with_record_batch_size(Some(137))
                .with_compression(compression)
                .finish(&mut frame)?;
            let cancel = Cancellation::default();
            let source = Source::open(
                Location::local(file.path().to_owned()),
                CsvOptions::default(),
                "",
                &cancel,
            )?;
            assert_eq!(source.info.total_rows, 2501);
            let page = load_source(Arc::clone(&source), 500, RowLimit::Paged(500), 0, &cancel)?;
            assert_eq!(page.frame.height(), 500);
            assert_eq!(page.cell(0, 0).0, "500");
            assert_eq!(page.cell(499, 0).0, "999");
            assert_eq!(page.profiles[0].dtype, "u64");
            let last = load_source(source, 999_000, RowLimit::Paged(500), 0, &cancel)?;
            assert_eq!(last.frame.height(), 1);
            assert_eq!(last.cell(0, 0).0, "2500");
        }
        let file = tempfile::NamedTempFile::new()?;
        let mut frame = df!("id"=>Vec::<u64>::new())?;
        IpcWriter::new(file.reopen()?).finish(&mut frame)?;
        let page = crate::data::load_page(file.path(), 0, 100)?;
        assert_eq!(page.total_rows, 0);
        assert_eq!(page.frame.width(), 1);
        assert_eq!(page.profiles[0].dtype, "u64");
        Ok(())
    }

    #[test]
    fn csv_index_respects_quotes_newlines_blanks_and_stable_schema() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("résultats.csv");
        let mut file = File::create(&path)?;
        writeln!(file, "id,message,flag")?;
        for row in 0..1251 {
            writeln!(
                file,
                "{row},\"line {row}\ncomma, and \"\"quote\"\"\",{}",
                row % 2 == 0
            )?;
            if row % 7 == 0 {
                writeln!(file)?;
            }
        }
        drop(file);
        let cancel = Cancellation::default();
        let source = Source::open(Location::local(path), CsvOptions::default(), "", &cancel)?;
        assert_eq!(source.info.total_rows, 1251);
        for offset in [0, 500, 1000] {
            let page = load_source(
                Arc::clone(&source),
                offset,
                RowLimit::Paged(500),
                0,
                &cancel,
            )?;
            assert_eq!(page.cell(0, 0).0, offset.to_string());
            assert_eq!(
                page.cell(0, 1).0,
                format!("line {offset}\ncomma, and \"quote\"")
            );
            assert_eq!(page.frame.height(), (1251 - offset).min(500));
            assert_eq!(page.profiles[0].dtype, "i64");
            assert_eq!(page.profiles[2].dtype, "bool");
        }
        let summary = overview(source, &cancel)?;
        assert!(summary.page.frame.height() <= 1000);
        assert_eq!(summary.windows, 16);
        let Distribution::Histogram { min, max, .. } = &summary.details[0].profile.distribution
        else {
            panic!("numeric")
        };
        assert_eq!(min, "0");
        assert_eq!(max, "1250");
        Ok(())
    }

    #[test]
    fn csv_options_text_inference_header_encoding_and_changed_files() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("data.tsv");
        std::fs::write(&path, b"10\t\xff\n20\tvalue\n")?;
        let options = CsvOptions {
            has_header: false,
            lossy_utf8: true,
            infer_types: false,
            ..Default::default()
        };
        let cancel = Cancellation::default();
        let source = Source::open(Location::local(path.clone()), options, "", &cancel)?;
        let page = load_source(Arc::clone(&source), 0, RowLimit::Paged(100), 0, &cancel)?;
        assert_eq!(page.total_rows, 2);
        assert_eq!(page.profiles[0].dtype, "str");
        assert_eq!(page.cell(0, 1).0, "�");
        std::fs::write(&path, "changed file\n")?;
        assert!(source.read_slice(0, 100, &cancel).is_err());
        let path = dir.path().join("data.csv");
        std::fs::write(&path, "a;b\r\n1;two\r\n")?;
        let source = Source::open(
            Location::local(path),
            CsvOptions {
                delimiter: Delimiter::Semicolon,
                ..Default::default()
            },
            "",
            &cancel,
        )?;
        let page = load_source(source, 0, RowLimit::Paged(100), 0, &cancel)?;
        assert_eq!(page.frame.width(), 2);
        assert_eq!(page.cell(0, 1).0, "two");
        Ok(())
    }

    #[test]
    fn cancellation_and_history_never_keep_signed_urls() -> Result<()> {
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(cancel.check().is_err());
        let url = Location::remote("https://example.com/file.parquet?token=secret")?;
        assert!(!url.rememberable());
        assert!(!url.display().contains("secret"));
        assert!(Location::remote("https://user:secret@example.com/file.parquet").is_err());
        assert!(Location::remote("s3://bucket/file.parquet")?.rememberable());
        assert!(
            Location::remote("abfss://container@account.dfs.core.windows.net/file.parquet")?
                .rememberable()
        );
        Ok(())
    }
    use crate::data::Distribution;
}
