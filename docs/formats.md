# Formats and reading behavior

[User guide](user-guide.md) · [Installation](installation.md) · [Keyboard shortcuts](keyboard-shortcuts.md)

## Supported sources

| Source | Core build | Cloud build | Notes |
| --- | :---: | :---: | --- |
| Local Parquet (`.parquet`, `.parq`) | ✓ | ✓ | Typed columns; paged row-range reads |
| Local Hive Parquet folder | ✓ | ✓ | `key=value` partitions; paged reads across files |
| Local Arrow IPC / Feather v2 (`.ipc`, `.arrow`, `.feather`) | ✓ | ✓ | IPC file format; paged record-batch reads |
| Local CSV / TSV (`.csv`, `.tsv`) | ✓ | ✓ | Configurable parsing; one indexing scan on open |
| Remote Parquet | — | ✓ | S3, Azure, GCS, and HTTP(S), including signed GET URLs |

Tablelamp opens one file, local Parquet folder, or remote object at a time. IPC streams, Feather v1, remote folders/CSV/IPC, Iceberg tables, and editing are not currently supported. See the [roadmap](roadmap.md) for planned work.

## Hive Parquet folders

Choose **Open Parquet folder…**, or pass a folder path to the executable. Tablelamp recursively discovers Parquet files and adds virtual columns from Hive paths:

```text
events/
  year=2025/region=North/part-001.parquet
  year=2026/region=South/part-002.parquet
```

Physical columns keep their file order; `year` and `region` follow them in partition-path order. Values are percent-decoded; `__HIVE_DEFAULT_PARTITION__` becomes null. Partition types are inferred across the manifest as signed 64-bit integer, finite float, Boolean (`true`/`false`), or text. Oversized integer identifiers stay text rather than losing precision. Dates remain text.

All files must share the same physical schema and ordered partition keys. Partition columns must be omitted from the files. Hidden/underscore entries, symlinks, and non-Parquet files are ignored. Discovery reads metadata, not every row; limits are **100,000 files**, **400,000 directory entries**, **64 nesting levels**, and a **64 MiB manifest/metadata estimate**.

Folders always open paged, even if **All** was saved globally. You can explicitly choose All afterward, subject to the usual memory checks. Files are read in path order and pages can cross partition boundaries. The manifest is a snapshot taken on open: reopen to see added files. Changes to a file being read are rejected; SQL checks the whole manifest before and after execution.

## SQL mode

Open **SQL…** above the table, or press **Ctrl+Shift+L** (**Cmd+Shift+L**). The opened source is registered as `data`. The default query previews 1,000 rows. Run one read-only **SELECT** or **WITH** query with **Ctrl+Enter** (**Cmd+Enter**). Column names with spaces need double quotes.

```sql
SELECT "region", COUNT(*) AS rows
FROM data
GROUP BY "region"
ORDER BY rows DESC
```

Queries use the **entire source**, independently of normal filters and page sorting. While SQL is active, those controls are disabled. SQL controls result column order and sorting; paging, charts, visibility, inspection, and copying work on the result. The gutter reads **Result row**, because derived rows do not necessarily correspond to an original record. **Exit SQL mode** restores the previous normal view. File details and File overview continue to describe the original source.

Parquet and Hive queries use lazy Polars scans with projection and predicate pushdown where supported. Local IPC uses a lazy scan. CSV/TSV and remote Parquet first stage the complete source into a private IPC file, preserving CSV settings and existing cloud credentials/range behavior; even a selective query on these sources can require a complete read. Remote folders and external SQL table functions are unsupported. See the [Polars SQL introduction](https://docs.pola.rs/user-guide/sql/intro/) for syntax and functions.

Each query runs in an isolated process. Results stream into an IPC file and are displayed in pages; a newly executed query starts paged when the saved choice is All. Paging an existing result reuses that file. Cancel or failure leaves the previous view usable. Query results are temporary snapshots and are recomputed on reopening a source; saved draft text is separate from the last successful query.

The worker monitors resident memory against **25% of available memory, capped at 1 GiB**, and is stopped if it exceeds that budget. Input/result batches are checked as well. This is a periodic safeguard, not a hard allocation limit: some [streaming operations can still need substantial memory](https://docs.pola.rs/user-guide/concepts/streaming/). Narrow a query or add LIMIT when needed. Each staged input and result file is capped at **2 GiB**; total query scratch space, including Polars spills, is monitored against **4 GiB**.

Scratch files live under the platform's per-user cache in `parquetviewer/queries`, and are removed when the result closes or execution fails/cancels. An abrupt crash can leave a temporary directory; close Tablelamp before removing abandoned directories. Signed URLs and profile names are sent to the worker through a pipe, not a saved request file. Signed sources still have no saved history/query. SQL requires a UTF-8 source path.

## CSV and TSV

Choose **CSV / TSV options…** from the menu before opening a source, or while viewing one. Select **Save and apply** to save the choices and reload the current CSV/TSV. Options are remembered per file and as defaults for new files.

| Option | Choices / behavior |
| --- | --- |
| Delimiter | Automatic, comma, tab, semicolon, or pipe. Automatic uses tab for `.tsv` and comma for `.csv`. |
| Header | The first record contains column names or data. |
| Encoding | Strict UTF-8 or explicit replacement of invalid bytes. Other encodings are not guessed. |
| Types | Infer from the first 1,000 records, or read every column as text. Date/time inference follows Polars. |

The inferred schema stays fixed across pages. If a later record conflicts with it, Tablelamp reports the parse error instead of silently replacing the value with null. Use **Read all columns as text** for mixed-type columns.

The first open scans records to count rows and creates a temporary disk-backed checkpoint index. Later pages reuse it without rescanning the prefix. The index is discarded when the source closes and rebuilt on a later launch. Quoted newlines, escaped quotes, CRLF, and bare blank lines are handled; bare blank lines are skipped. Individual records over **16 MiB** and pages exceeding the buffer budget are rejected.

## Remote Parquet

Use a **cloud** build, then choose **Open remote Parquet…** from the menu. Enter one object URL and open it. An S3 profile can be selected in the same panel. You can also pass the URL to the executable:

```sh
parquetviewer 's3://my-bucket/data/example.parquet'
```

| Provider | Example |
| --- | --- |
| Amazon S3 | `s3://my-bucket/data/example.parquet` |
| Azure | `az://my-container/data/example.parquet` or an `abfs://` / `abfss://` URL |
| Google Cloud Storage | `gs://my-bucket/data/example.parquet` |
| HTTP(S) | `https://example.com/data/example.parquet` or a signed GET URL |

These are example locations; use your own object and existing provider configuration. Core builds explain that remote support is unavailable. [Build or install the cloud variant](installation.md#core-or-cloud).

### Credentials and saved URLs

Credentials come from existing provider environment settings. S3 also reads standard shared AWS credentials/config, using the panel's profile or `AWS_PROFILE`, then `default`. Exported access credentials take precedence. SSO, role-assumption, or credential-process profiles require temporary credentials exported from your existing login.

Azure/GCS use the environment and credential providers supported by the project's `object_store` dependency. Tablelamp has no credential-entry form and does not store credential secrets.

URLs with a query or fragment, including signed URLs, are excluded from history and preferences. Window titles and transport errors omit secret query strings. Ordinary remote URLs without a query/fragment can be remembered.

### Transfers and cancellation

Tablelamp requests the footer and required column-chunk byte ranges rather than eagerly downloading the whole object. The per-source in-memory range cache is capped at **32 MiB**; each request has a **30-second timeout**. Servers must support byte ranges and provide an ETag, version, or Last-Modified value so pages cannot silently mix object versions.

Reopen a changed source to refresh its metadata and clear the cache. **Cancel** interrupts active network requests; decoding checks cancellation between chunks. Whole-file filters and SQL staging can fetch substantial data even though ordinary browsing uses range reads.

## Charts and statistics

Table charts use the loaded page, capped at its first **1,000 source-order rows**. In All mode this remains a sample. Expanded column details reuse exactly the same rows without another source read. Sorting changes displayed order but not the sample.

| Type | Header plot | Expanded view |
| --- | --- | --- |
| Integers, floats, decimals | 16-bin histogram | 32 bins; mean, median, interpolated quartiles, sample standard deviation |
| Dates, timestamps, time, durations | Chronological histogram | More bins with typed observed endpoints |
| Boolean | False / true bars | Counts and percentages among non-null values |
| Strings, categorical, enum | Four frequent values plus Other | Up to 20 categories plus Other; text lengths and empty-string counts where relevant |
| Binary, list, array, struct, all-null | No distribution plot | Type/null information; inspect individual cells |

Constant numeric columns use one bin. Nulls are counted separately. NaN and infinity are excluded from numeric calculations and reported separately. Distinct counts are exact within the sampled rows, excluding nulls.

Numeric statistics and histogram coordinates use floating-point arithmetic. Very close large integers can share a bin or lose precision in derived statistics. Sorting uses native Polars types, preserving integer precision; histogram hover labels retain exact observed typed endpoints. Bin starts are approximate. Sample columns larger than **8 MiB** skip chart/distinct calculations with an explanation.

The [file overview](user-guide.md#get-a-broader-file-overview) instead samples up to 1,000 rows across 16 evenly spaced windows of the unfiltered source. It is a stratified preview, not a random sample or an exact global report. Sources of at most 1,000 rows are summarized completely.

## Filters

See [Filter the file](user-guide.md#filter-the-file) for the interaction steps. Each column can have one condition; up to eight conditions combine with **AND**.

| Column type | Conditions / input |
| --- | --- |
| Text, categorical, enum | Case-sensitive **Contains** / **Equals**; an empty equality value matches empty strings |
| Numeric | **Equals** / **Between (inclusive)**; native integer/decimal precision |
| Boolean | **Equals**, then **True** or **False** |
| Date | Equality/range; `YYYY-MM-DD` |
| Timestamp | Equality/range; ISO timestamp with an explicit offset, or `YYYY-MM-DD HH:MM:SS` interpreted as UTC |
| Time | Equality/range; `HH:MM:SS[.fraction]` |
| Duration | Equality/range; integer in the column's stored unit |
| Every type | **Is null** / **Is not null** |

Value comparisons exclude nulls. Bounds finer than a decimal/timestamp column's stored precision are rejected.

Filters scan the entire source in background batches of at most **4,096 rows**, recording matching original row numbers in a temporary disk-backed index (**8 bytes per match**). The exact match count becomes available after the scan. Cancel keeps the prior view. Filtered pages read the source ranges needed for those matches; sorting remains limited to the loaded matching page.

Plots summarize the first 1,000 loaded matches at most; **File overview** still describes the unfiltered source. Definitions are saved for remembered sources, but reopening must rebuild the temporary index. Incompatible filters are cleared with a notice if the schema changed.

Large row groups and remote sources can make scans expensive. This column-filter path does not use SQL or predicate pushdown; SQL mode is a separate alternative. Batches exceeding one quarter of the memory budget are rejected. **All** may reject a filtered result using the original source's conservative estimate; paged viewing remains available.

## Large files and memory

Parquet row counts and row-group counts come from its footer, so opening it does not require a full row-count scan. The reader decodes requested row ranges on a background worker. IPC reads intersecting record batches and skips earlier ones; large or compressed batches can still require extra decoding memory.

The app retains one current page, with a brief overlap while its replacement loads. Both rows and columns are virtualized, so only the visible table area is rendered. Paging bounds the number of retained rows, **not their byte size**: wide schemas, large values, and compressed pages/row groups can still need significant memory.

**All** checks conservative source estimates and a bounded decoded sample before loading the complete dataframe. Its budget is **25% of currently available memory, capped at 2 GiB**, accounting for the retained page. Native Windows/macOS memory queries and detectable Linux cgroup v2 limits are respected. Unknown availability uses a **64 MiB** budget. A rejected load leaves the existing page usable, and the check runs again for every source.

Rendering and summaries stay bounded in All mode, and large sorts run on the worker. The estimate reduces out-of-memory risk but cannot guarantee it: later oversized values, decoder allocations, malformed files, or another program consuming memory can exceed an estimate. Use paged mode for very large files.
