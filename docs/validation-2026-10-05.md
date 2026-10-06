# Validation on 2026-10-05

Linux x64 / Arch workstation, Ryzen 9 9950X, Rust 1.99.0. These are local development packages. Windows/macOS runtime and signing checks remain pending; no native executor or GitHub remote is configured in this workspace.

## Checks

- Core and cloud release builds completed. Formatting and Clippy with warnings denied passed for all targets, including cloud/UI tests.
- The cloud/UI suite passed 31 tests: 22 library tests and 9 viewer tests. Coverage includes Parquet, compressed/uncompressed IPC, quote-aware CSV/TSV indexing and options, cell inspection/copy, overview sampling, search/jump, preference restoration, history clearing, chart expansion, resize/scrollbars and remote ranges/cancellation.
- Native UI checked on an isolated 1280×900 X11 display: Parquet/CSV/IPC tables, column search/jump, cell inspector, file details, parsing options, file overview and expanded charts. Column detail fills the app window and preserves its size/fullscreen state. The Boolean overview shows the most common value.
- Native HTTP opening checked both from the command line and the remote-source form against a loopback range server. For a 198,194-byte, 50,000-row Parquet fixture, initial paging made 9 range requests totaling 106,759 bytes. Page two reused the cache with no additional requests. The synthetic signed query was omitted from the title and saved preferences/history.
- HTTP integration tests also reject changed objects and servers that ignore ranges, and cancel an outstanding request. S3/Azure/GCS provider paths compile, but have not been exercised against live accounts.
- The extracted Linux installer was run with isolated user directories and six pre-existing format defaults. Every default was preserved; desktop entry validation and installed binary comparison passed.
- The updated cloud build was installed to `~/.local/bin/parquetviewer`, with existing format defaults preserved, and the installed copy opened the IPC fixture successfully.
- Native Windows/macOS release checks are listed in [packaging/TESTING.md](../packaging/TESTING.md). CI builds both variants on all four platform/CPU targets when dispatched.

## Indicative measurements

Optimized Linux executable sizes: approximately **38.5 MiB core** and **42.6 MiB cloud**. Networking remains an optional Cargo feature; the core dependency tree excludes object_store and the additional transport/provider dependencies.

The bundled `sample` example generated six-column fixtures in 25,000-row batches. Each measurement below starts a new headless `inspect` process after a warm-up read. Read time is reported by `inspect` around opening, decoding and profiling; peak RSS comes from Linux child-process resource usage. CSV/TSV times include scanning all records to create the temporary index. These smoke measurements exclude the native window/GPU and are not performance guarantees.

| Source | Total rows | File size | Requested rows | Read time | Peak RSS |
| --- | ---: | ---: | --- | ---: | ---: |
| Parquet | 50,000 | 193.5 KiB | 1–500 | 1.6 ms | 13.9 MiB |
| Arrow IPC | 50,000 | 2.6 MiB | 1–500 | 1.3 ms | 20.9 MiB |
| CSV | 50,000 | 1.8 MiB | 1–500 | 8.0 ms | 12.9 MiB |
| TSV | 50,000 | 1.8 MiB | 1–500 | 7.8 ms | 13.6 MiB |
| Parquet | 10,000,000 | 34.9 MiB | 9,999,501–10,000,000 | 9.9 ms | 50.2 MiB |

The native cloud process reached approximately 205 MiB RSS on the isolated Xvfb/Mesa software-rendered display while checking tables and panels. That includes the GPUI renderer and its caches; it is a different measurement from headless data reading and does not establish hardware-GPU desktop memory usage.

Reproduce the larger source read:

```sh
cargo run --locked --release --example sample -- /tmp/large.parquet 10000000
cargo run --locked --release --example inspect -- /tmp/large.parquet 9999500
```

The sample generator refuses to overwrite an existing file. Fixture content, unusually large values, row-group/batch layout, available memory and filesystem cache all affect latency and peak memory. The existing All-mode budget reduces OOM risk without guaranteeing decoder allocation limits.
