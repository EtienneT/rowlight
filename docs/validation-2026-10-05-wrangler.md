# Column exploration and filters: validation on 2026-10-05

Linux x64 / Arch, Ryzen 9 9950X, Rust 1.99.0. Native Windows/macOS runtime checks remain pending. The existing platform CI and packaging paths are retained.

## Implemented

- Bounded column summaries: numeric mean, median, interpolated quartiles and sample standard deviation; Unicode text lengths and empty-string counts; Boolean counts and percentages. Numeric summaries exclude nulls, NaN and infinity and use floating-point arithmetic. Typed endpoints and filter comparisons preserve native precision.
- Distinct null, empty-string, NaN and infinity presentation, plus sampled header null percentages and bars. Expanded charts use a statistics sidebar inside the existing window.
- Column resizing, fitting visible values, rectangular mouse/keyboard selection, cell inspection and bounded TSV copying. Row copying includes visible headers and preserves displayed order.
- Searchable column visibility controls with original file order preserved. Widths and visibility restore from per-source preferences.
- Typed whole-file filters with AND across up to eight columns. Background scans retain matching source row numbers in a temporary disk index, support cancellation, and expose an exact result count only after completion. Paging retains original source positions. Filters restore on reopen; invalid saved filters are cleared with a notice.

## Checks

- `cargo fmt --all --check` passed.
- `cargo clippy --locked --all-targets -- -D warnings` passed for the core build.
- `cargo clippy --locked --all-targets --features cloud,ui-tests -- -D warnings` passed.
- `cargo ui-test` passed **41 tests**: 29 library tests and 12 viewer tests. Added coverage includes native integer/decimal/temporal filter precision, combined text/Boolean/null filters across Parquet/IPC/CSV, sparse matching pages and cancellation, remote HTTP filtering, statistics and clipboard bounds, keyboard selection and copying, resizing/alignment, visibility restoration, saved filter pagination, and stale-schema recovery.
- Native Linux UI checked on an isolated 1280×800 X11 display: expanded chart/sidebar, column menus, typed range filters, filter chips, exact matching counts, pagination and original row numbers. Filtering the 50,000-row demo by `id` from 49,000 through 50,000 produced 1,001 matches and three 500-row pages. Production preferences were not used by the preview.
- The installed optimized copy opened that saved filtered view successfully. Adding `active = false` through the Boolean buttons produced 192 matches, agreeing with an independent calculation from the fixture generator; the combined filters were persisted in the isolated preferences.

## Development loop

All implementation iterations used the cached development profile and stable cloud features. Compiler checks took approximately **0.5 seconds**, incremental executable rebuilds **2.5–2.8 seconds**, and the final UI test build **2.6 seconds** before roughly 2.3 seconds of test execution. No dependency cache was cleared and no global compiler flags were changed. The single final optimized cloud build took **61 seconds**. The executable installed at `~/.local/bin/parquetviewer` matches that build by SHA-256; its command-line startup and native preview both succeeded. Its size is 45,480,712 bytes (43.4 MiB). File-association registrations and production preferences were preserved.

## Scope and limits

Sorting applies to the loaded matching page; filters apply to the entire source. Statistics use at most the first 1,000 displayed matches. File overview remains an explicitly labeled sample of the unfiltered source. These scopes are visible in the app and described in the README.

Filtering uses bounded slice scans without predicate pushdown. Large row groups can be decoded repeatedly, and remote scans can download substantial data. Matching row IDs consume eight bytes per match on disk; decoder and page budgets still apply. The conservative All-mode guard can reject a filtered result based on the original source estimate; paging remains available. Decoder allocation safeguards reduce memory risk without guaranteeing an OOM-proof process.

Live S3/Azure/GCS accounts, native Windows/macOS execution, and signed distribution packages have not been verified here.

## Scroll correction

GPUI's default single-axis overflow behavior remapped vertical wheel input to the horizontal container, and horizontal input to the nested row list. Both table scrollers now restrict input to their own axis. A new 40-column UI regression reproduced the old behavior (ordinary wheel input moved columns by 78 pixels), then passed after the fix. It checks line/pixel vertical scrolling, repeated input at the end of the rows, and horizontal gestures without row movement. The full cloud/UI suite now passes **42 tests** (29 library, 13 viewer), alongside formatting and Clippy with warnings denied. The installed cloud binary is rebuilt after this correction.

## Keyboard navigation

Home/End reaches the first/last displayed row of the current page. Page Up/Down moves the selection and viewport by a screenful, with one row of overlap. Ctrl/Cmd+Left/Right reaches the first/last visible column. Shift extends row/screen/column navigation within the loaded page.

Ctrl/Cmd+Page Up/Down and Alt+Left/Right change file pages, retaining the selected column. Ctrl/Cmd+Home/End reaches the first row of the first page or last row of the last page. Cmd+Up/Down adds familiar macOS aliases. These destinations follow active filters and use bounded page reads, including when jumping to the end. Repeated keys during a load cannot start another concurrent read.

Ctrl/Cmd+G selects the page-number field; Enter submits the destination and returns focus to the table. Ctrl/Cmd+A selects loaded rows and visible columns with the existing copy limits intact. Esc clears selection or closes a chart/tool. F1 and the menu open an in-app shortcut reference. Navigation works before the first cell click and leaves focused inputs' editing shortcuts intact.

The cloud/UI suite now passes **44 tests** (29 library, 15 viewer). New real keyboard-dispatch tests cover row/screen/column boundaries and scrolling, Shift selection, clipboard bounds, initial focus, page-field focus/editing/Enter, tool isolation, F1, first/last pages, repeated keys during a load, filtered paging, All mode, and empty results. Formatting and core/cloud Clippy with warnings denied pass. The cached test rebuilds took roughly 1.7–2 seconds, and the development executable rebuilt in 2 seconds; dependency features and compiler flags were unchanged.

Native Linux checks on the isolated 1280×800 display confirmed the F1 reference layout and a Ctrl+End jump in the 50,000-row demo: page 100 loaded only 500 rows, source row 50,000 was selected and visible at the bottom, and the final page/scroll position was saved in the isolated preferences. The installed optimized cloud app is updated after validation.
