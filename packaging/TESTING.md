# Native release checks

## SQL error recovery on 2026-10-07

The bounded error display passed the focused native GPUI regression on Linux and
Windows. The Linux workspace run passed 67 tests, plus formatting and all-feature
Clippy. Tests keep the editor and Run button usable at 1,000 × 500 with a large
diagnostic, scroll and copy the full diagnostic, and successfully run a corrected
query. Common error explanations are checked without exposing engine plans in
the summary.

An isolated running Linux app also passed missing-column and syntax errors,
expanded details at 1,000 × 500, and correcting the query to count a synthetic
100,000-row Hive dataset. Windows coverage here is native headless GUI/worker
testing; desktop rendering and macOS checks remain part of release validation.

## Release checklist

Run these against the extracted/installed package on each target OS, including both Mac architectures. Record the version, OS/CPU, archive checksum, and result. CI covers native compilation, core/GUI logic, and packaging; it does not substitute for desktop integration testing.

1. Launch from the desktop application entry rather than the source checkout. Confirm a usable window, readable text, picker, drag/drop, and close/quit.
2. Install/register the viewer. Verify the first-launch offer appears and declining does not change the default. Quit/reopen to verify the decline is remembered. Reopen the offer through the app menu, then choose it as the default for `.parquet` and `.parq`, then double-click a file whose name contains spaces, Unicode, and `%`. On macOS repeat while the application is already running, including during a read. Verify the requested document opens.
3. Open a wide file with more columns than fit. Scroll both ways, resize/fullscreen the window, and verify the names, types, plots, and cells stay aligned. Click the horizontal scrollbar after several seconds idle. Verify the vertical scrollbar stays visible and scrolls rows at the beginning, middle, and end of the horizontal range, with plots both shown and hidden.
4. Change page size, hide plots, sort a column, navigate, and scroll. Quit and reopen the same file. Verify preferences and its view return from the documented user preferences folder, without writing inside the app's install directory. In **Settings → Appearance**, check System, Light and Dark, including restart persistence. System should follow live OS appearance changes on Windows/macOS and Linux without Omarchy; on Omarchy it should follow live palette changes. Explicit Light/Dark choices should stay fixed when the OS changes. On macOS, check that the native window chrome matches an explicit choice and resumes OS following in System mode.
5. Load All for a small file and verify the row count/sorting. Try an oversized file and verify a memory-budget message with the previous page still usable. Confirm the plot/count scope says first 1,000 rows when sampled.
6. Click numeric, temporal, Boolean and string charts; verify column details fill the app's existing window without changing its size or fullscreen state, with meaningful axes, hover counts/percentages, and the sample/null/distinct cards. Check the small charts' hover borders leave space around their content. Return via Back and Esc, including from an already fullscreen table. Use pagination page numbers and the page field.
7. Verify Windows scaling on different-DPI monitors, and verify Cmd+O/Cmd+Q plus Finder document opening on macOS. Confirm the artifact's architecture matches the machine.
8. Upgrade the installation, then uninstall. Verify launchers/registrations are removed and unrelated file associations remain intact. User settings should remain unless explicitly deleted.

Before public distribution, verify Developer ID/notarization on macOS and Authenticode signing on Windows, then repeat with the exact downloaded artifact on a clean machine. Current build scripts produce unsigned/local development artifacts.

## New source and overview checks

- Open IPC/Feather v2 (compressed and uncompressed), CSV and TSV, including Unicode paths and final/empty pages. Large reads remain paged. Confirm CSV quoted newlines/CRLF, delimiter/header/encoding and all-text settings; reopen to verify preferences.
- Find a late column by name/type. Click a cell, select text, copy it, and check full nested values and the explicit oversized-value limit. File details must show schema and Parquet compression/encodings/row groups.
- Check Settings, recent files, remote sources, CSV options, SQL and shortcuts in dialogs; search, column visibility, filters, file details and cell inspection in side drawers. The table should remain visible behind each tool. Close with the button, Esc and the backdrop; the table's scroll, selection and sorting must be preserved. Tab should stay within the open tool, and Enter in a text field should not dismiss it unexpectedly. Resize to a small window and verify tool contents remain reachable.
- Build the file overview on sorted data. Verify the scope label, sampled counts and expanded chart; Back returns to the overview. Cancel a long source scan; opening another source while busy must cancel/queue correctly.
- Reopen a recent file and verify its page/scroll/sort. Clear history, close and reopen; the history stays empty.
- Cloud download: open HTTP(S), S3, Azure and GCS with existing credentials. Verify range-only traffic, timeout/cancellation, repeated-page cache reuse and changed-object rejection. Signed URLs and secrets must not appear in saved preferences/history or error messages. SSO/role profiles require exported temporary credentials.
- Build both core and cloud packages. Verify Open With registration for all local formats without replacing existing defaults, on Windows, macOS and Linux.

## Windows x64 validation on 2026-10-06

Source commit `558e4f47b9fa916f3f2a4aa415a5c758f821aeac` was tested natively on Windows 11 Pro (build 26200), using Rust 1.99.0, Visual Studio Build Tools 2022 and Inno Setup 6.7.3. No application fixes were needed during this validation.

- Formatting and core/all-feature Clippy passed with warnings denied. All 46 tests passed: 29 data/remote/settings tests, 15 headless viewer tests and two development-runner tests. This confirms the earlier Windows HTTP fixture fix with blocking accepted sockets.
- Parquet, IPC, CSV and TSV smoke tests passed. A Parquet path containing spaces, Unicode and `%` opened correctly; paged reads and All were checked.
- Both core/cloud ZIPs and installers built. Extracted executables started, were x64, included the GPUI PerMonitorV2 manifest and did not depend on external Visual C++ runtime DLLs. Executable sizes were 31.6 MiB (core) and 35.7 MiB (cloud).
- The core portable app and installed cloud app rendered the Parquet fixture in the native desktop session. The file picker opened/cancelled. Charts expanded inside the existing window; Home/End, paging, preference restoration and small-file All worked. A 40-column CSV kept ordinary wheel scrolling vertical, exposed later columns horizontally, and kept headers/cells aligned after resizing.
- Per-user core installation, upgrade to cloud, and uninstall passed. Open With registration covered all seven supported extensions, command quoting was correct, existing file defaults stayed unchanged, and uninstall removed the viewer registration while preserving saved preferences. The temporary test tasks and app processes were removed; the build cache and portable apps were retained.

Still requiring manual release checks: Explorer default selection/double-click, native drag/drop, physical monitors with different DPI, real S3/Azure/GCS credentials, and signed distribution. The HTTP transport tests use local fixtures, and these packages remain unsigned development artifacts.

## Appearance checks on 2026-10-07

The saved System/Light/Dark setting passed 50 tests on Linux and 48 on Windows, plus core/cloud Clippy with warnings denied. Tests cover native appearance callbacks, fixed overrides, shared control/chart colors, settings clicks, persistence and older preference files. Linux also checks missing/invalid Omarchy palettes and legacy-path precedence.

The running Linux development and optimized apps passed Light/Dark switching, saved Light on restart and returning to the current Omarchy palette, using isolated preferences. The installed Linux app and retained Windows cloud portable build were updated; the Windows cloud ZIP and installer rebuilt successfully. Windows desktop capture was unavailable in the current remote session, so native Windows appearance switching and macOS appearance/window-chrome checks remain pending. GitHub Actions stayed disabled; these checks used local builds.

## Table and tool checks on 2026-10-07

The fixed scrollbar layout and tool overlays passed 52 tests on Linux and 50 on Windows, plus formatting and Clippy with warnings denied. Regression tests click the visible vertical scrollbar at the beginning, middle and end of a 40-column table, with plots shown and hidden. All utility overlays fit a 1,000 × 500 window, keep the table visible and preserve scroll/sort/selection when dismissed through the backdrop.

The running Linux development and optimized apps passed both scrollbars, Settings Light/Dark controls, search with text input, cell inspection, shortcuts and Esc dismissal on an isolated X11 display. The installed Linux app was updated with the tested optimized executable. The Windows cloud ZIP and installer rebuilt, its extracted executable passed the startup help check, and the retained cloud portable app was updated. These Windows UI checks exercise compiled logic; desktop overlay checks and native macOS checks remain pending. No GitHub Actions runs were used.

## SQL and Hive release checks

- Open a local Hive folder with matching schemas and nested `key=value` partitions through the picker, command line and drag/drop. Verify virtual integer/float/Boolean/text columns, percent-decoded values and the Hive null marker. Metadata should show original rows, file count and row groups. Pages must cross files correctly. Add/change a partition and reopen.
- Save All on a small file, then open/reopen a large folder: it must start paged. Explicit All on the folder still uses the memory guard.
- Run SELECT/WHERE/ORDER BY, grouped counts, WITH, transformed columns and an empty result. Queries must use the whole source rather than the current page or normal filters. Result row numbers and chart scope must be clear; column sorting/filtering must be disabled until Exit SQL restores the previous normal view.
- Page/hide/resize result columns, save an invalid draft, quit and reopen. Recompute the last successful query and restore its separate view; the draft must remain editable. Cancel a long query, provoke a memory-budget failure and check that the previous page remains usable with no orphan worker. Query cache files should disappear when the result closes.
- Check compressed IPC with temporal/nested columns, CSV/TSV parsing options, and cloud Parquet with a signed URL and existing provider profile. SQL may stage the entire CSV/remote source; verify cancellation and avoid private URLs in preferences/logs.
- Run these on Windows and both macOS architectures with the downloaded package. Native builds and headless UI tests do not prove picker/desktop behavior.

## SQL and Hive validation on 2026-10-07

The SQL/Hive changes passed all 64 workspace tests on Linux and all 62 on Windows 11 x64, plus formatting and core/all-feature Clippy with warnings denied. The new integration tests launch the actual SQL worker: full-source ordering/transformation, grouped/CTE/empty results, Hive partition predicates and type inference, compressed IPC with temporal/nested columns, CSV parsing settings, signed HTTP staging, changed-object rejection, memory termination and cancellation. Headless viewer tests cover SQL ownership of filtering/sorting, separate saved normal/result views and drafts, keyboard submission with the editor focused, recovery from an invalid saved query, and folder paging despite saved All.

An isolated Linux X11 window opened a synthetic 100,000-row/two-file Hive folder at 500 rows despite an All preference. A SQL query selected the 2026 partition, ordered its rows descending and returned 1,200 rows; paging/restart restored its result view. A grouped query returned two 50,000-row counts. Chart expansion retained the window size and showed the result sample; the SQL dialog remained usable at 1,000 × 500. Ctrl+Enter worked in the multiline editor, and Exit SQL restored normal browsing. Graceful quit and leaving SQL removed the current result's scratch files. The worker also remained usable after a Cargo rebuild replaced the running Linux executable.

Windows checks ran natively with four Cargo jobs and a retained build cache. These establish executable/worker behavior and headless UI logic; Windows desktop picker/SQL interaction and native macOS validation remain pending. GitHub Actions remained disabled.

The optimized Linux cloud executable (71.9 MiB) passed the same folder/query/paging/restart interaction with isolated preferences and was atomically installed locally. The Windows cloud executable (67.0 MiB) passed startup help and a real Hive grouped-count worker run; the resulting IPC file was read back successfully. Its cloud ZIP/setup rebuilt successfully and the retained cloud portable app was updated. These are unsigned development builds. The Windows release build took 6 minutes 49 seconds; build plus packaging took 429.3 seconds. Native macOS and Windows desktop interaction remain pending.
