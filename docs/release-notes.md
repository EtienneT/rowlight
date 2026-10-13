# Rowlight 0.1.0-alpha.1

Shed light on your data. A small, read-only desktop viewer for Parquet, Arrow IPC
/ Feather v2, CSV and TSV, built with Rust, GPUI Kit and Polars.

Open a file or Hive Parquet folder, see typed columns and distributions, inspect
cells, and browse large sources in pages. SQL mode queries the opened source as
`data`; filters and sorting return when you exit SQL. Settings and views persist.
On Omarchy the app follows your palette; elsewhere it follows system light/dark
appearance, with an explicit choice in Settings.

## Downloads

- **Omarchy / Arch x64:** native package; install using
  `sudo pacman -U ./rowlight-bin-0.1.0.alpha.1-1-x86_64.pkg.tar.zst`.
  AUR publication is pending; future updates currently require downloading the
  newer package.
- **Linux x64:** archive with a per-user installer; glibc 2.39+ (Ubuntu 24.04
  baseline), Wayland or X11, Vulkan graphics driver.
- **Windows x64:** per-user setup executable or portable ZIP; Windows 10+ and a
  DirectX 11 graphics driver.

Cloud downloads also open remote Parquet from S3, Azure, GCS and HTTP(S), using
existing provider credentials. The executable is still named `parquetviewer` for
compatibility. Existing preferences and file associations are preserved.

This is an **unsigned early alpha**. Windows may show an unrecognized-publisher
warning. Verify download hashes against `SHA256SUMS`. This verifies file integrity,
not publisher identity.

## Scope and limitations

- Charts describe displayed rows or an explicitly labelled bounded sample.
- Column sorting applies to loaded rows; SQL can sort the complete query source.
- All mode and SQL have memory/storage guards; prefer paging for large sources.
- CSV/TSV indexing and SQL on CSV/remote Parquet can require a complete scan.
- Iceberg, remote folders, editing and IPC streams are not supported.
- macOS is experimental community tooling; no official Mac downloads.
- On Linux, after accepting the file-association offer, click the table before
  using keyboard shortcuts.
- Native Windows build, startup, installation, upgrade and removal were tested.
  Some Windows desktop interactions, mixed-DPI displays and real cloud accounts
  still need manual validation; see the linked validation record.

See [installation](https://github.com/EtienneT/rowlight/blob/main/docs/installation.md),
the [user guide](https://github.com/EtienneT/rowlight/blob/main/docs/user-guide.md),
and [validation evidence](https://github.com/EtienneT/rowlight/blob/main/packaging/TESTING.md).
Bug reports with small synthetic examples are welcome. Third-party license texts
and source links are included in each download.
