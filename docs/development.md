# Development loop

Start from the project directory after installing the native GPUI prerequisites:

```sh
cargo dev -- samples/demo.parquet
```

The Rust `xtask` helper builds the cloud development app, opens it and watches its sources. Save a Rust file to rebuild and restart. It waits for a 300 ms quiet period so editor saves are combined. Changes during a build are picked up afterward. A compiler error leaves the working preview open and waits for the next edit. Close the app or press Ctrl+C to stop the helper and its owned processes.

The supplied file/URL argument is reused on restart. Development preferences keep page size, plot visibility and per-file view positions separately from the installed app. They live in `target/dev-preview/cloud/settings.json` (or `core/settings.json`); the association offer is suppressed there. The preview follows the same desktop theme. The helper sets `PARQUETVIEWER_SETTINGS_PATH` only for its child app; the ordinary platform preferences remain the default elsewhere.

The first dependency build is still substantial. Once cached, app edits use Cargo's normal incremental, unoptimized development profile with debug symbols disabled. Keep the feature set and target directory consistent. Cargo's [profile documentation](https://doc.rust-lang.org/cargo/reference/profiles.html) explains the compilation/runtime tradeoffs. The size-optimized thin-LTO release profile is reserved for packaging and performance measurements.

## Commands

| Command | Use |
| --- | --- |
| `cargo dev -- FILE` | Build, preview and restart on source saves, including cloud support |
| `cargo dev --core -- FILE` | Same loop for local-only features |
| `cargo dev --once -- FILE` | Build and run once, without a watcher |
| `cargo dev-build` | Build the cloud development app without launching |
| `cargo quick-check` | Compiler feedback without executable code generation/linking |
| `cargo ui-test` | Run the data/preferences and headless viewer tests |
| `cargo build --release --features cloud` | Build a distribution/performance binary |

`FILE` can be a local path or a remote Parquet URL in cloud mode. Quote paths containing spaces. The development helper does not print file/URL arguments. Arguments after `--` are passed to the app; for example `cargo dev --once -- --help` checks startup without a graphical display.

The watcher tracks `src/`, the root manifest/lockfile, `build.rs`, `.cargo/config.toml`, the toolchain and embedded Windows resources. Documentation/data-file edits do not rebuild the app. New/deleted source files and atomic editor replacements are detected. Changes to the helper itself require restarting `cargo dev`.

Nested Cargo builds remove the helper's injected package metadata from their environment. This keeps dependency fingerprints compatible with ordinary `cargo quick-check`, `cargo dev-build` and test commands; TLS dependencies no longer rebuild because the caller changed.

The helper runs a copy of each successful executable. This permits rebuilding on Windows while the previous preview is open. Copies are removed when their processes end. Build and preview process groups are owned by the helper, so stopping the loop does not target other viewer instances. The helper's small dependencies are outside the viewer's runtime graph, and no globally installed watcher is required.

## Measurements on 2026-10-05

Linux x64, Ryzen 9 9950X, Rust 1.99.0, warm dependency cache:

| Operation | Observed time |
| --- | ---: |
| Earlier thin-LTO release rebuilds after app edits | 48–49 s |
| Compiler check after changing a UI string | 0.57 s |
| Incremental cloud build after changing a UI string | 1.71 s |
| Build with no source changes | 0.17 s |
| Watcher save → rebuilt/restarted preview | 2.52 s |

After fixing inherited Cargo package metadata, alternating warm `cargo quick-check`, `cargo dev-build` and a copied-executable startup check took 0.27 s, 0.23 s and 0.51 s respectively, without recompiling TLS dependencies.

The initial development build after switching to the current feature set took 11.7 seconds, including several uncached GPUI crates. Numbers vary by machine, edit, dependency features, filesystem and concurrent load. Dependency/toolchain/profile changes can cause much larger rebuilds; do not run `cargo clean` during routine iteration. Changing global Rust compiler/linker flags also invalidates dependency artifacts.

The live native check intentionally introduced a compiler error, verified that the original preview remained open, then corrected it with a real UI edit. The preview restarted, kept 1,000 rows/page and collapsed plots, and left production preferences unchanged. The development app also opened the typed IPC fixture correctly. Runner tests cover argument preservation and source creation/deletion/atomic replacement; Clippy denies warnings. Native Windows/macOS checks remain pending; CI includes the runner's native build/tests and a copied-executable startup check on both platforms.

## Build and validation

The workspace pins **Rust 1.99.0**, edition **2024**, and the application lockfile. GPUI Kit brings its matching GPUI dependencies. Polars' dataframe and I/O crates are enabled; its lazy query engine and SQL are omitted, and cloud transport is optional.

Install the [native platform prerequisites](https://gpui-kit.com/docs/installation/) before building. Cargo uses four jobs to keep dependency compilation bounded. Development builds omit debug symbols; release builds optimize for size, use thin LTO, and strip symbols. See [installation and native packaging](installation.md) for each platform's scripts.

For routine iteration, use the stable cloud feature set and cached commands above. Documentation edits do not need an application rebuild. CI additionally checks the core variant and development runner:

```sh
cargo fmt --all --check
cargo quick-check
cargo ui-test
cargo clippy --locked --all-targets --features cloud -- -D warnings
# Core-only checks; switching features can rebuild dependencies.
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
# Development runner:
cargo clippy --locked --package xtask --all-targets -- -D warnings
cargo test --locked --package xtask
```

The data tests cover Parquet row groups, IPC batches, CSV parsing/paging, typed sorting, empty/final pages, nulls and non-finite values, nested inspection, sampled overviews, whole-file filters, clipboard limits, and preference restoration. HTTP integration tests check real range requests, bounded cache reuse, object changes, unsupported servers, and cancellation. Headless GPUI tests drive paging, selection, column visibility/sizing, axis-independent scrolling, resize alignment, chart expansion, saved views, and the opt-in association prompt without changing host defaults.

For native desktop validation, follow [the release checklist](../packaging/TESTING.md). Compilation and headless UI tests do not establish desktop integration on Windows/macOS.

## Continuous integration

The [Native builds workflow](https://github.com/EtienneT/rowlight/actions/workflows/ci.yml) uses standard GitHub-hosted runners for Linux x64, Windows x64, and Apple Silicon/Intel macOS. These runner minutes are [free for public repositories](https://docs.github.com/en/actions/reference/runners/github-hosted-runners). The workflow skips every job while the repository is private, even if enabled. A maintainer must enable the workflow in GitHub after publishing before these checks can run.

Once public and enabled, source pushes to `main` and pull requests run development-profile checks. README/guide-only pushes skip the matrix, and new commits cancel superseded runs. Core lint still checks code without cloud features; cloud/UI lint and all workspace tests use one feature set. Native dependency caches are kept after failures and saved only from `main`, so pull requests reuse them without creating a cache for every branch. Compilation remains bounded to two jobs per runner.

Optimized packages are built only when a maintainer chooses **Run workflow → packages**. Leave that checkbox off for checks alone. To request packages with the GitHub CLI:

```sh
gh workflow run ci.yml --ref main -f packages=true
```

Package runs check the code first, then build both core and cloud variants on each platform. Uploaded packages expire after seven days. Routine checks have a 30-minute timeout per platform; package runs allow 90 minutes. The size-optimized release profile is unchanged.

Cold builds compile GPUI, Polars and their native dependencies separately on each OS/CPU. Caches reuse dependencies, but the application still needs compilation/linking after source changes. Thin LTO and single-codegen-unit release optimization also add time. Building two release variants on all four platforms for every push was the main avoidable cost of the original workflow.

To reproduce the routine lint/test commands locally:

```sh
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features --no-fail-fast
```

## Demo and measurements

```sh
cargo run --locked --example sample -- samples/demo.parquet
cargo dev -- samples/demo.parquet
# Generate a larger file in 25,000-row batches; never overwrites an existing file.
cargo run --locked --release --example sample -- /tmp/large.parquet 10000000
# Inspect a single page without a window, optionally starting at a late row.
cargo run --locked --release --example inspect -- /tmp/large.parquet 9999500
# Exercise guarded All mode without a window.
cargo run --locked --release --example inspect -- /path/to/file.parquet all
```

On this Linux workstation (2026-10-04), the optimized executable was about 37 MiB with gpui-omarchy. Reading the final 500 rows of the synthetic 10-million-row, 35 MiB fixture took about 8 ms with a warm filesystem cache, with 48 MiB peak resident memory for the headless `inspect` process. These measurements exclude the native window and GPU; file contents and row-group layout affect performance.

The [2026-10-05 validation and measurements](validation-2026-10-05.md) cover the additional formats, core/cloud sizes, native UI, HTTP range/cache checks and installer default preservation. The [column exploration and filter validation](validation-2026-10-05-wrangler.md) covers the added statistics, selection, visibility, whole-file filters and cached development timings.

## Code layout

- `src/data.rs`: page reads, typed sorting, summaries, bounded cell formatting.
- `src/source.rs`: local source metadata, IPC batch paging, CSV/TSV parsing and disk-backed row index.
- `src/remote.rs`: optional conditional cloud/HTTP byte ranges, bounded caching and request cancellation.
- `src/viewer.rs`: window state, background tasks, themed controls, virtual table, plots.
- `src/association.rs`: opt-in platform file associations (no startup default changes).
- `src/settings.rs`: validated preferences and atomic per-user persistence.
- `src/main.rs`: startup, path argument, shortcuts and application lifecycle.
- `xtask/`: development-only source watcher, incremental preview builds and process cleanup; excluded from the viewer's runtime dependencies.
- `packaging/` and `scripts/`: platform icons, file registration, macOS app bundle, Windows installer, and Linux desktop integration.
- `.github/workflows/ci.yml`: native checks for all four platform/CPU targets and optional manual packages, with pinned action revisions and a public-repository guard.

For future additions that keep the app focused, see [the small-viewer roadmap](roadmap.md).

MIT license. Dependencies retain their own licenses.
