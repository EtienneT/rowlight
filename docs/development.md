# Development loop

Start from the project directory after installing the native GPUI prerequisites:

```sh
cargo dev -- docs/demo/demo.parquet
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
| `cargo ui-test` | Run data/preferences, headless viewer, and real SQL-worker integration tests |
| `cargo build --release --features cloud` | Build a distribution/performance binary |

`FILE` can be a local file/folder path or a remote Parquet URL in cloud mode. Quote paths containing spaces. The development helper does not print file/URL arguments. Arguments after `--` are passed to the app; for example `cargo dev --once -- --help` checks startup without a graphical display.

The watcher tracks `src/`, the root manifest/lockfile, `build.rs`, `.cargo/config.toml`, the toolchain and embedded Windows resources. Documentation/data-file edits do not rebuild the app. New/deleted source files and atomic editor replacements are detected. Changes to the helper itself require restarting `cargo dev`.

Nested Cargo builds remove the helper's injected package metadata from their environment. This keeps dependency fingerprints compatible with ordinary `cargo quick-check`, `cargo dev-build` and test commands; TLS dependencies no longer rebuild because the caller changed.

For a small UI change, keep `cargo dev` running, save the edit, and inspect the restarted preview. Run `cargo ui-test TEST_FILTER` for the affected behavior, then format and lint the settled change. A compiler check immediately followed by a development build usually repeats work; choose the check when you only need diagnostics, and the build when you need to see the app. Full native release/installer validation belongs at release milestones rather than on every edit. [AGENTS.md](../AGENTS.md) records this workflow for coding agents.

The helper runs a copy of each successful executable. This permits rebuilding on Windows while the previous preview is open. Copies are removed when their processes end. Build and preview process groups are owned by the helper, so stopping the loop does not target other viewer instances. The helper's small dependencies are outside the viewer's runtime graph, and no globally installed watcher is required.

## Build and validation

### Toolchain and checks

The workspace pins **Rust 1.99.0**, edition **2024**, and the application lockfile. GPUI Kit brings its matching GPUI dependencies. Polars' dataframe, I/O, lazy streaming engine and read-only SQL are enabled with explicit features; cloud transport is optional. Adding SQL requires a one-time dependency rebuild on each platform; normal source changes continue to use cached incremental dev builds.

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

The data tests cover Parquet row groups, IPC batches, CSV parsing/paging, typed sorting, empty/final pages, nulls and non-finite values, nested inspection, sampled overviews, whole-file filters, clipboard limits, and preference restoration. SQL integration tests start the real executable in worker mode and cover full-source filtering/order, grouping, CTEs, empty results, Hive partitions, CSV options, compressed IPC and typed/nested values, signed HTTP staging, cancellation and memory termination. The test command also builds that executable; keep it current before running a filtered headless SQL test. HTTP integration tests check real range requests, bounded cache reuse, object changes, unsupported servers, and cancellation. Headless GPUI tests drive paging, selection, column visibility/sizing, axis-independent scrolling, resize alignment, chart expansion, saved views, SQL mode ownership/restoration, folder paging with saved All, and the opt-in association prompt without changing host defaults.

For native desktop validation, follow [the release checklist](../packaging/TESTING.md). Compilation and headless UI tests do not establish desktop integration. Linux and Windows are maintained; macOS is experimental community tooling.

## Continuous integration

The [Native builds workflow](https://github.com/EtienneT/tablelamp/actions/workflows/ci.yml) uses standard GitHub-hosted runners for Linux x64 and Windows x64. macOS is outside the maintained matrix. These runner minutes are [free for public repositories](https://docs.github.com/en/actions/reference/runners/github-hosted-runners). Checks are enabled for the public repository; the workflow skips every job while the repository is private.

Source pushes to `main` and non-draft pull requests run development-profile checks. README/guide-only pushes skip the matrix, and new commits cancel superseded runs. Core lint still checks code without cloud features; cloud/UI lint and all workspace tests use one feature set. Native dependency caches are kept after failures and saved only from `main`, so pull requests reuse them without creating a cache for every branch.

Cargo parallelism is bounded for the [public runner hardware](https://docs.github.com/en/actions/reference/runners/github-hosted-runners):

| Platform | Cargo build jobs | Runner RAM |
| --- | ---: | ---: |
| Linux x64 | 4 | 16 GB |
| Windows x64 | 4 | 16 GB |

These are starting limits, not measured speedups. Check the first public runs for time and memory pressure before raising them further. Local builds retain the four-job default; `CARGO_BUILD_JOBS` can override it for a particular machine.

Optimized packages are built only when a maintainer chooses **Run workflow → packages**. Leave that checkbox off for checks alone. After an explicit release request, build packages with the GitHub CLI:

```sh
gh workflow run ci.yml --ref main -f packages=true
```

Package runs check the code first, then build the cloud variant on each maintained platform. Core packaging remains an optional source-build choice. Uploaded packages expire after seven days. Routine checks have a 30-minute timeout per platform; package runs allow 90 minutes. The size-optimized release profile is unchanged.

Windows package runs also test installation, upgrade from the first alpha,
startup, removal, and preservation of file defaults and preferences on the clean
runner. To validate existing packages without rebuilding them, manually run
**Release package validation** with the originating Native builds run ID.
For installer-only changes, enable **rebuild_installer** to compile the current
Inno Setup recipe against that run's existing application binary. This runs no
Cargo build and provides a tested installer preview as a workflow artifact.

Cold builds compile GPUI, Polars and their native dependencies separately on each OS/CPU. Caches reuse dependencies, but the application still needs compilation/linking after source changes. Thin LTO and single-codegen-unit release optimization also add time. Building two release variants on all four platforms for every push was the main avoidable cost of the original workflow.

For further tuning, measure a build with `cargo build --locked --bin parquetviewer --features cloud --timings`. Rust 1.99 selects bundled LLD on Linux x64. macOS uses Apple's [modern linker](https://developer.apple.com/documentation/xcode-release-notes/xcode-15-release-notes). Benchmark `lld-link` on Windows before changing its linker, including executable resources and static-CRT packages. Linker changes should be tested on the target OS.

A compiler cache such as [sccache](https://github.com/mozilla/sccache/blob/main/docs/Rust.md) can reuse dependency compilation when Cargo's target cache misses. It cannot cache final application linking or Rust incremental builds, so keep local incremental development enabled and evaluate it separately for CI. Avoid adding a second cache until timings show dependency compilation is still the bottleneck.

To reproduce the routine lint/test commands locally:

```sh
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features --no-fail-fast
```

## Demo data

```sh
cargo dev -- docs/demo/demo.parquet
# Generate a separate synthetic fixture:
cargo run --locked --features cloud --example sample -- /tmp/tablelamp-demo.parquet 1000
# Generate a larger file in 25,000-row batches; never overwrites an existing file.
cargo run --locked --release --example sample -- /tmp/large.parquet 10000000
# Inspect a single page without a window, optionally starting at a late row.
cargo run --locked --release --example inspect -- /tmp/large.parquet 9999500
# Exercise guarded All mode without a window.
cargo run --locked --release --example inspect -- /path/to/file.parquet all
```

## Code layout

- `src/data.rs`: page reads, typed sorting, summaries, bounded cell formatting.
- `src/source.rs`: local source metadata, IPC batch paging, CSV/TSV parsing and disk-backed row index.
- `src/hive.rs`: bounded local Parquet manifests, partition inference and sliced cross-file reads.
- `src/query.rs`: read-only validation, isolated SQL worker, monitored budgets and streamed IPC results.
- `src/viewer/query.rs`: query dialog, mode transitions and folder picker.
- `src/remote.rs`: optional conditional cloud/HTTP byte ranges, bounded caching and request cancellation.
- `src/viewer.rs`: window state, background tasks, themed controls, virtual table, plots.
- `src/association.rs`: opt-in platform file associations (no startup default changes).
- `src/settings.rs`: validated preferences and atomic per-user persistence.
- `src/main.rs`: startup, path argument, shortcuts and application lifecycle.
- `xtask/`: development-only source watcher, incremental preview builds and process cleanup; excluded from the viewer's runtime dependencies.
- `packaging/` and `scripts/`: platform icons, file registration, macOS app bundle, Windows installer, and Linux desktop integration.
- `.github/workflows/ci.yml`: native checks for the two maintained platform/CPU targets and optional manual packages, with pinned action revisions and a public-repository guard.

For distribution, see [release preparation](releasing.md) and [platform support](platforms.md).

For future additions that keep the app focused, see [the small-viewer roadmap](roadmap.md).

MIT license. Dependencies retain their own licenses.
