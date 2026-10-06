# Keeping the viewer focused

The app remains a read-only window into one tabular source: understand its schema, preview its rows, and inspect distributions.

## Implemented

- Find a column by name/type and jump without reordering.
- Inspect/copy complete cell values, with explicit large/deep-value display limits.
- File details: source path, schema, size, writer, compression/encodings and row groups or record batches.
- Rich column summaries beside expanded charts: finite numeric mean/median/quartiles/sample deviation, text lengths/empty counts and boolean percentages.
- Distinct null/empty/NaN/infinity styles and sampled null percentage bars with hover counts.
- Resizable columns, bounded auto-sizing, keyboard/mouse selection and bounded TSV copying.
- Searchable column visibility controls with original file order and saved widths/visibility.
- Whole-source column filters with cancellable bounded scans, disk-backed matching indexes, exact post-scan pagination and original source row numbers.
- Recent files with restored views and Clear history.
- Explicit bounded file overview across evenly spaced windows, with sampled null/distinct/range/category statistics and expanded charts.
- Local Arrow IPC / Feather v2 with record-batch paging.
- CSV/TSV with saved parsing options, stable sample-inferred schemas or all-text mode, quote-aware row counts and a disk-backed paging index.
- Optional cloud download: S3, Azure, GCS and HTTP(S) Parquet, conditional range reads, a bounded memory cache, cancellation, timeouts and existing provider credentials. Signed URLs are never saved in history.

## Future separate milestone

Apache Iceberg is a table snapshot backed by manifests, multiple data files and delete handling, potentially with a catalog ([Iceberg specification](https://iceberg.apache.org/spec/)). It needs a native table reader and snapshot selection rather than another file extension. Iceberg remains deferred, as recommended, and is not supported by the current app.

Keep sorting scope and exact vs sampled statistics visible. Measure package size, first-page latency and peak memory when adding sources. Keep SQL, editing, joins, ETL and automatic full-file profiling out of the core app.
