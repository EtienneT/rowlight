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
