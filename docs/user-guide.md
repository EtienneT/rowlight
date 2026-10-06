# Rowlight user guide

Rowlight is a focused native Rust app: open one tabular source, see its columns and types, and browse its rows. Supports local Parquet, Arrow IPC / Feather v2, CSV and TSV; an optional cloud build adds remote Parquet.

Targets Windows x64, macOS on Apple Silicon and Intel, and Linux x64.

Built with [GPUI Kit](https://gpui-kit.com/) 0.7, [gpui-omarchy](https://github.com/huacnlee/gpui-omarchy) 0.2, and [Polars](https://docs.pola.rs/) 0.55. The app is read only. Local sources stay local; remote sources are fetched only when explicitly opened.

## Use

```sh
cargo run --locked --release -- /path/to/file.parquet
```

Use the top-left menu → **Open file**, Ctrl/Cmd+O, or drop a file onto the window. On first launch the app offers to associate `.parquet` files; **No thanks** remembers the choice, and **Associate Parquet files…** in the menu can reopen the offer.

- Column headers show the name, exact Polars type, page distribution, and null count.
- Visible columns always stay in their original file order. Use **Show / hide columns…** to choose a subset, or a column’s **⋯** menu to hide it; **Show all** restores the complete schema. The menu’s **Show column plots** option switches between compact and expanded headers; names, types, and null counts remain visible.
- Drag a header’s right edge to resize its column; double-click the edge or choose **Fit visible values** to size it from the header and up to 40 displayed values. Widths are bounded to 120–800 pixels and saved per file.
- Click a cell to select it, Shift-click or Shift+arrows to extend a rectangular selection, and use arrows to navigate. Double-click or Enter opens the cell inspector. Ctrl/Cmd+C copies selected cells as TSV; click the source-row gutter to select whole rows, or use Ctrl/Cmd+Shift+C to copy selected rows with visible column names. Copy preserves displayed row order and is limited to 10,000 cells and 1 MiB; a cell exceeding the inspector limit is rejected rather than silently truncated. Tabs, newlines and quotes are escaped in multi-cell TSV; single cells copy their raw value. Nulls copy as `null`, empty strings as empty fields.
- Click a column name to cycle ascending, descending, and source order. This sorts **the loaded rows** (the displayed page, or the complete file in All mode); row numbers retain their original file positions. Nulls sort last.
- Browse in pages of **100, 500, or 1,000 rows**, or select **All** when the file fits the memory budget. The bottom status bar uses GPUI Kit pagination with page numbers, first/last destinations, Previous/Next arrows, and a page field (Enter / Go). File size, format and column counts, row range, read-only state, and summary/sort scope live there too.
- The shaded **# / Source row** gutter contains original file row numbers, separate from the Parquet columns.
- Click a plot to give its column center stage: a detail view fills the existing app window with a large chart, axes, hover counts/percentages, 32 histogram bins or up to 20 categories, and a scrollable statistics panel. Numeric columns add mean, median, interpolated quartiles and sample standard deviation; text adds empty-string counts and Unicode character lengths; booleans add false/true percentages among non-null values. Statistics reuse the bounded sample, exclude null/NaN/infinity from numeric calculations, and use floating-point arithmetic for numeric summaries. Typed histogram endpoints remain exact. **Back to table** or **Esc** restores the table (or the file overview when opened there). The window size and fullscreen state stay unchanged.
- Ordinary wheel scrolling moves through rows, even in wide tables and at the end of the rows. Horizontal gestures or the horizontal scrollbar move through columns without moving rows. Scrollbars stay visible when content overflows; headers and cells refresh together when resizing or entering fullscreen.

| Shortcut | Action |
| --- | --- |
| Ctrl+O / Cmd+O | Open a file |
| Ctrl+F / Cmd+F | Find a column |
| Alt+Left / Alt+Right | Previous / next page |
| Ctrl+Q / Cmd+Q | Quit |
| Arrows / Shift+arrows | Navigate / extend the grid selection |
| Home / End | First / last row of the current page |
| Page Up / Page Down | Move one screenful within the current page |
| Shift+Home / End / Page Up / Page Down | Extend selection to the destination |
| Ctrl+Left / Right / Cmd+Left / Right | First / last visible column; add Shift to extend selection |
| Ctrl+Page Up / Page Down / Cmd+Page Up / Page Down | Previous / next file page, preserving the selected column |
| Ctrl+Home / End / Cmd+Home / End | First row of the first page / last row of the last page |
| Ctrl+G / Cmd+G | Focus and select the page number; type a destination and press Enter |
| Ctrl+A / Cmd+A | Select all loaded rows and visible columns |
| Enter | Inspect the selected cell |
| Ctrl+C / Cmd+C | Copy selected cells |
| Ctrl+Shift+C / Cmd+Shift+C | Copy selected rows with headers |
| Esc | Clear selection or return from a chart or tool panel |
| F1 | Show the keyboard shortcut reference |

The menu also includes **Keyboard shortcuts…**. On macOS, Cmd+Up/Down additionally reaches the first/last file page. Table navigation works immediately after opening and leaves focused input controls' editing shortcuts intact. Home/End and Page Up/Down stay inside the current loaded page; file-page navigation follows active filters. Selection stays within loaded rows, and existing clipboard limits still apply after Select All. File-page shortcuts use normal paging and never switch to All mode.

## Overview tools and formats

The menu includes **Find a column** (Ctrl/Cmd+F), **File details**, **File overview**, **Recent files**, **CSV / TSV options**, and **Open remote Parquet**. Find matches column names or types and jumps without changing file order. Double-click a cell or select it and press Enter to inspect and copy its complete scalar/nested representation; the inspector is selectable and read only. Values exceeding 65,536 characters or 32 nesting levels are clearly marked as truncated, and copying uses only the displayed text.

File details show the full source path, schema in file order, size, writer, Parquet compression/encodings and row-group sizes (or IPC record batches). They use already loaded metadata. Recent files restore the saved view; **Clear history** also suppresses re-recording the current source until another source is opened.

**File overview** explicitly reads up to 1,000 rows from 16 evenly spaced windows, including the beginning and end of the source. It shows sampled null percentages, distinct values, ranges/top categories, and expandable distributions. These are a stratified preview, not a random sample or exact global statistics. Sources of 1,000 rows or fewer are summarized completely. The action runs in the background and can be cancelled.

Arrow IPC / Feather **v2 file format** preserves typed columns. The loader skips preceding record batches and reads only intersecting batches. IPC streams and Feather v1 are not supported. Large/compressed batches can still require extra decoding memory and are guarded by a conservative batch budget.

CSV/TSV options control comma/tab/semicolon/pipe delimiters, the header, strict/replacement UTF-8, and sample-based type inference or all-text reading. Automatic chooses tab for `.tsv`, comma for `.csv`. Other encodings are not silently guessed. The first open scans records to count rows and builds a temporary disk-backed checkpoint index; later pages reuse that index without scanning the file prefix. Quoted newlines, escaped quotes, CRLF and bare blank lines are handled consistently (bare blank lines are skipped). The schema is inferred from the first 1,000 records and stays fixed across pages. If later values conflict, switch to all-text mode; parse failures are reported, not silently converted to nulls. Options are saved per file and as defaults; date/time inference follows Polars when type inference is enabled. Records over 16 MiB and pages beyond the buffer budget are rejected. The index is discarded when the source closes, and rebuilt on a later launch.

### Optional cloud build

```sh
cargo run --locked --release --features cloud -- 's3://bucket/file.parquet'
# Cloud downloadable package:
./scripts/package-linux.sh --cloud
# macOS: ./scripts/package-macos.sh --cloud
# Windows: ./scripts/package-windows.ps1 -Cloud
```

Core downloads include all local formats. Cloud downloads add S3 (`s3://`), Azure (`az://`, `abfs://`), GCS (`gs://`) and HTTP(S) Parquet, including signed GET URLs. They use the same app identity and preferences. Use **Open remote Parquet** or pass a URL on the command line. Only footer and required column-chunk byte ranges are fetched; there is no eager whole-object download. A per-source in-memory range cache is capped at 32 MiB, and each request has a 30-second timeout. Servers must honor ranges and provide an ETag/version/Last-Modified so pages cannot silently mix object versions. Reopen a changed source to refresh metadata and clear its cache. Cancel interrupts active network requests; CPU decoding checks cancellation between chunks.

Credentials come from existing provider environment settings. S3 also reads standard shared AWS credentials/config, using the optional profile field or `AWS_PROFILE` / `default`; exported access credentials take precedence. Profiles requiring SSO, role assumption or a credential process need temporary credentials exported from your existing login. Azure/GCS use the environment and credential providers supported by object_store. No credential-entry form or secret storage is added. Signed URLs are not saved to history, preferences or window titles, and transport errors omit their secret query strings. Remote URLs without query/fragment can be saved as recent sources. Additional cloud transport/provider dependencies remain excluded from the core build.

## Read-only column filters

Use a column’s **⋯ → Filter…** menu. Text supports case-sensitive contains/equality (including the empty string), numbers support equality/inclusive ranges, booleans support true/false, and dates/timestamps/time/durations support typed comparisons. Every type supports null/not-null. Up to eight filters on different columns combine with AND. Removable filter chips and **Clear all** are visible above the table.

Filters apply to the **entire source**, using background scans of at most 4,096 rows at a time and a temporary disk-backed index of matching original row numbers (8 bytes per match). The status bar shows scan progress; the exact matching count and pagination appear after the scan completes. Cancel keeps the prior view. Pages read only the source ranges needed for their matches. Filtered views keep original source row numbers; sorting remains limited to the loaded matching page. Plots/statistics summarize that page’s first 1,000 matches, while **File overview** continues to describe a sample of the unfiltered source.

Scanning can be slow for large files or large Parquet row groups, and remote filters may fetch substantial data. This is bounded slice scanning, without a query engine or predicate pushdown. Decoding safeguards still apply, and scan batches are rejected if their decoded size exceeds one quarter of the memory budget. **All** retains its conservative memory guard and may reject a filtered result based on the original source’s estimate; paging remains available.

Filter definitions are saved per remembered source; reopening rebuilds their temporary index. Signed remote URLs retain their existing exclusion from history/preferences. Saved filters that no longer fit a changed schema are cleared with a notice. Dates use YYYY-MM-DD, timestamps accept ISO with an explicit offset or YYYY-MM-DD HH:MM:SS interpreted as UTC, times use HH:MM:SS[.fraction], and durations use integers in the stored unit. Numeric comparisons preserve native integer/decimal precision; bounds finer than a decimal/timestamp column’s precision are rejected.

The grid renders `null` in muted italic text, empty strings as **(empty string)** in the accent color, and NaN/infinity in the warning color. Each header shows its sampled null percentage and a small null bar; hover for the sample size, empty-string and non-finite counts. These distinctions do not alter values.

## Preferences and Omarchy theme

Page size (including All), plot visibility, the file-association prompt decision, and window dimensions are saved automatically in the platform's user configuration directory:

| Platform | Preferences |
| --- | --- |
| Linux | `${XDG_CONFIG_HOME:-~/.config}/parquetviewer/settings.json` |
| Windows | `%APPDATA%\parquetviewer\settings.json` |
| macOS | `~/Library/Application Support/parquetviewer/settings.json` |
 The viewer also remembers page position, sort column/direction, column widths and visibility, whole-file filters, and horizontal/vertical scroll position for the 20 most recently opened files. Reopening one restores its view, with page positions clamped if the file changed. Files with non-UTF-8 paths still open, but their per-file position is not recorded in JSON. Starting without a path continues to show the file picker/empty window.

Preferences use atomic replacement; invalid or older settings are handled with defaults and validation. Resize/scroll updates are debounced and flushed on close. Fullscreen and maximized dimensions do not replace the saved window size; your window manager still controls placement and tiling.

The app follows Omarchy's current theme through gpui-omarchy, including changes while it is running. On current Omarchy this reads `~/.local/state/omarchy/current/theme/colors.toml` and `theme.name`; older installations have a legacy config-directory fallback. Missing/invalid themes fall back to Tokyo Night, which is also the default on Windows and macOS without an Omarchy theme. The application does not modify your desktop theme files.

## Double-click opening on Linux

From a graphical Linux session:

```sh
./scripts/install-linux.sh
```

This builds the release executable, installs it to `~/.local/bin/parquetviewer`, and registers a user-level application, icon, and MIME association for `.parquet` and `.parq`. No administrator privileges are needed. The launcher uses an absolute executable path, including when the file manager has a different `PATH`. It offers a first-launch prompt before changing the default Parquet application, and records the previous default under `${XDG_DATA_HOME:-~/.local/share}/parquetviewer/previous-default`.

For a downloaded Linux archive, extract it and run `./scripts/install-linux.sh` inside the extracted `parquetviewer` directory; it uses the included binary and needs no Rust toolchain. To produce that archive from source, run `./scripts/package-linux.sh` (or add `--cloud` for remote support). Use `./scripts/install-linux.sh --cloud` to build/install that variant from source. Existing defaults for all advertised formats are preserved during registration.

Required desktop utilities: `xdg-mime`, `update-mime-database`, and `update-desktop-database`. Use `--skip-build` after a release build is already available. Windows and macOS have separate native packaging scripts described below.

To restore a previous default, read `previous-default` and use `xdg-mime default PREVIOUS.desktop application/vnd.apache.parquet application/x-parquet`. Installed files can be removed from `~/.local/bin` and the corresponding application, icon, and MIME directories under your XDG data directory.

## Windows

Requires Windows 10 or later, x64, and a DirectX 11 graphics driver. For source builds, install Rust's MSVC toolchain, Visual Studio C++ Build Tools with a Windows SDK, CMake, and [Inno Setup 6](https://jrsoftware.org/isinfo.php). The Windows SDK supplies both the resource and shader compilers. Then run in PowerShell:

```powershell
./scripts/package-windows.ps1
```

This creates `dist/parquetviewer-windows-x64-setup.exe` and a portable ZIP; `-Cloud` adds `-cloud` to their names. The release executable uses a static C runtime, embeds its icon/version information, and declares per-monitor DPI awareness and long-path support. The installer registers Open With support for `.parquet`, `.parq`, `.ipc`, `.arrow`, `.feather`, `.csv` and `.tsv`, creates a Start menu entry, and installs per user without elevation. Uninstall removes its registrations and executable; your preferences remain.

On first launch, choose **Choose in Windows Settings** and select `.parquet` / `.parq` on the app’s Default Apps page. On Windows 10 use Default Apps → Choose default apps by file type. Alternatively use **Open with → Choose another app → Always**. As described in [Microsoft’s default-app documentation](https://learn.microsoft.com/windows/apps/develop/windows-integration/default-apps-platform), modern Windows owns the default-app choice; the installer registers the viewer without overwriting the protected user selection. The portable ZIP opens files through the picker, drag/drop, or command-line paths and does not register itself.

## macOS

Requires macOS 15 or later. Build natively on an Apple Silicon or Intel Mac with Rust and Xcode Command Line Tools, matching [GPUI Kit's platform requirements](https://gpui-kit.com/docs/installation/):

```sh
./scripts/package-macos.sh
# Or build, install to ~/Applications, and register with Finder:
./scripts/install-macos.sh
```

The package contains `dist/Parquet Viewer.app` and a ZIP for the current CPU architecture; `--cloud` adds `-cloud` to the ZIP name. The bundle includes its icon, declares Parquet, IPC/Feather v2 and CSV/TSV document types, and handles Finder's document-open events both at launch and while running. Filenames with spaces, percent signs, and Unicode are decoded as file URLs. A document opened during a read is queued and displayed afterward. Cmd+O/Cmd+Q and the native File/application menus are supported.

Drag the `.app` from an extracted ZIP into Applications (or use the user installer). The first-launch **Make default** button uses a bundled Swift helper and Apple’s [NSWorkspace association API](https://developer.apple.com/documentation/appkit/nsworkspace/setdefaultapplication(at:toopen:completion:)); users do not need Swift installed. Declining leaves the existing default in place. Finder’s **Get Info → Open with → Change All** is also available.

These scripts produce local/CI packages, not publicly signed releases. macOS uses an ad hoc signature; public downloads need Developer ID signing and notarization. Windows public distribution should sign the application and installer. No signing credentials are required for local source builds, and none are included in the project.

## Platform validation

CI builds, runs the data/preferences and headless UI tests, and packages on Linux x64, Windows x64, macOS Apple Silicon, and macOS Intel. Packages are attached to successful workflow runs. Unicode filenames are exercised through the real Parquet reader on each platform.

Linux has been built and run locally. GitHub Actions is configured to build and package all four platform/CPU targets; inspect [the workflow results](https://github.com/EtienneT/rowlight/actions/workflows/ci.yml) for the current status. Native Windows/macOS desktop behavior still needs the release checks below. Before distributing, complete [the native release checks](../packaging/TESTING.md) on those systems. A Linux test pass does not establish native Windows or macOS runtime support.

## Distribution plots

Plots and null/non-finite counts summarize **the loaded page**, up to **the first 1,000 rows** in All mode. They use source row order, so changing the sort does not change the sample. The scope is stated in the window, including when plots are hidden. This sample is not an estimate of the entire file. Expanded charts reuse exactly the same source sample and do not trigger another file read. Distinct counts are exact within that sample; numeric bin starts are approximate, with exact observed typed endpoints in hover details.

| Data type | Plot |
| --- | --- |
| Integers, floats, decimals | 16-bin histogram; one bin for constant values |
| Dates, timestamps, time, duration | Chronological histogram with typed endpoints |
| Boolean | False / true frequency bars |
| Strings, categorical, enum | Four most frequent values plus Other |
| Binary, list, array, struct, all-null | Type and null count; no distribution plot |

Nulls are counted separately. NaN and infinity are excluded from numeric histograms and counted in the header. Numeric row sorting uses native Polars types, preserving large integer precision. Histograms use floating point coordinates, so close values beyond floating point precision may share a bin. Cells have bounded previews of 256 characters, including nested values; the inspector shows more. Binary values are copied as hexadecimal.

## Large files

`ParquetReader::with_slice` reads the requested row range. Row counts and row-group counts come from the footer; the app does not scan the whole file to compute them. The reader uses low-memory, sequential decoding on a background worker. A window allows one outstanding read and retains one current page, with a brief overlap while its replacement is produced. Both axes of the table are virtualized: only visible rows and columns are rendered.

Paged mode bounds retained row count, rather than bytes: very wide schemas, large individual values, and Parquet page / row-group decompression can still require significant memory.

**All** checks conservative footer estimates and a bounded decoded sample before loading the complete dataframe. The budget is 25% of currently available memory, capped at 2 GiB, accounting for the retained previous page and using native memory queries on Windows/macOS and respecting detectable Linux cgroup v2 limits. Unknown available memory uses a 64 MiB budget. Loads above the budget are rejected with an explanation and leave the existing page usable. The check runs again for every file, including when All was remembered from a prior session. Large sorts also run on the background worker. Row/column rendering and distribution summaries remain bounded in All mode. Columns whose sample exceeds 8 MiB skip charts/distinct calculations with a visible explanation.

The memory check reduces OOM risk; it cannot guarantee safety against malformed files, unusually large later values, decoder allocations, or other programs consuming memory during a read. Use paging for very large files. Editing and SQL are outside this viewer's scope. Iceberg remains a separate table-browsing milestone.

## Development

Rust **1.99.0**, edition **2024**. The toolchain and application lockfile are checked in. Only Polars' dataframe, Arrow/Parquet and I/O crates are enabled; its lazy query engine and SQL are omitted. Networking is an optional feature. GPUI Kit manages its matching GPUI dependencies.

Install GPUI Kit's [native platform prerequisites](https://gpui-kit.com/docs/installation/) and a working Vulkan driver on Linux. This workspace uses four Cargo jobs to keep the heavy first compilation within reasonable memory. Development builds omit debug symbols; release builds optimize for size, use thin LTO, and strip symbols.

For UI iteration, use the development preview:

```sh
cargo dev -- samples/demo.parquet
```

It uses cached incremental builds with cloud features enabled, watches app sources, and restarts after successful changes. Failed builds leave the existing preview open. The same file argument and saved view are restored, with separate preferences under `target/dev-preview/cloud/settings.json`. It skips the association prompt. Close the preview or press Ctrl+C to stop. `--core` selects local-only development, and `--once` runs without watching. No Python, cargo-watch or additional global installation is required.

Use `cargo quick-check` for fast compiler feedback, `cargo dev-build` to build without launching, and `cargo ui-test` for the library/UI tests. Packaging and data benchmarks continue to use release builds. See [the development workflow and timings](development.md).

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo test --locked --features ui-tests --bin parquetviewer
cargo clippy --locked --all-targets --features cloud -- -D warnings
cargo test --locked --features "ui-tests cloud" --all-targets
cargo clippy --locked --package xtask --all-targets -- -D warnings
cargo test --locked --package xtask
cargo build --locked --release
```

Tests also cover compressed/uncompressed IPC batches, quote-aware CSV paging/options, nested cell inspection, sampled source overviews, column search/jump, copying, history clearing and preference restoration. HTTP integration tests exercise real range responses, typed decoding, bounded cache reuse, object changes, rejection of servers without ranges and cancellation. The data tests cover Parquet row-group boundaries, final and empty pages, invalid input, nulls, non-finite numbers, large integer sorting, temporal / decimal / nested types, and non-UTF-8 paths on Linux. The headless GPUI tests drive column sorting, horizontal virtualization, and actual background page navigation. They expand a column's details without changing the window size or fullscreen state, and exercise the opt-in association offer without changing the test host’s defaults. They also reopen a window to verify saved preferences, page/sort restoration, collapsed headers, All mode, and background sorting. Core tests verify atomic preference round trips, compatibility/invalid settings, bounded All-mode summaries, and memory-budget rejection.

A second UI regression test expands the window from 1280 to 3840 pixels, checks newly visible headers against their cells, shrinks it again, and clicks the horizontal scrollbar after five seconds idle.

### Demo and measurements

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

### Code layout

- `src/data.rs`: page reads, typed sorting, summaries, bounded cell formatting.
- `src/source.rs`: local source metadata, IPC batch paging, CSV/TSV parsing and disk-backed row index.
- `src/remote.rs`: optional conditional cloud/HTTP byte ranges, bounded caching and request cancellation.
- `src/viewer.rs`: window state, background tasks, themed controls, virtual table, plots.
- `src/association.rs`: opt-in platform file associations (no startup default changes).
- `src/settings.rs`: validated preferences and atomic per-user persistence.
- `src/main.rs`: startup, path argument, shortcuts and application lifecycle.
- `xtask/`: development-only source watcher, incremental preview builds and process cleanup; excluded from the viewer's runtime dependencies.
- `packaging/` and `scripts/`: platform icons, file registration, macOS app bundle, Windows installer, and Linux desktop integration.
- `.github/workflows/ci.yml`: formatting, Clippy, data tests and UI tests, and native packages for all four platform/CPU targets, with pinned action revisions.

For future additions that keep the app focused, see [the small-viewer roadmap](roadmap.md).

MIT license. Dependencies retain their own licenses.
