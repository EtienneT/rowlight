# Release validation

## 0.1.0-alpha.2

The maintained release targets are Linux x64 and Windows x64. The published
packages are unsigned. macOS is experimental community tooling.

| Coverage | Linux x64 | Windows x64 |
| --- | --- | --- |
| Formatting, lint and application regressions | Passed | Passed |
| Package version, product identity and license notices | Passed | Passed |
| Installation, upgrade, removal and file-default preservation | Passed | Passed |
| Paging, Hive folders and SQL worker regressions | Passed | Passed |
| Rendered package table, distributions and SQL | Passed | Pending |
| Picker, drag/drop and live appearance changes in the release package | Not repeated | Pending |
| Physical mixed-DPI displays | Not assessed | Pending |
| Live S3, Azure and GCS accounts | Not assessed | Not assessed |

The Linux archive uses an Ubuntu 24.04 build baseline and passes the glibc 2.39
compatibility guard. The Arch package contains the same application executable.
Windows packages include the static C runtime and preserve the legacy executable,
application and settings identifiers. Versioned downloads, checksums and source
provenance are attached to the GitHub prerelease.

The release was checked on 2026-10-14. [Native builds](https://github.com/EtienneT/tablelamp/actions/runs/38634583215)
passed formatting, core/cloud lint, workspace tests, Unicode-path data smoke tests
and optimized cloud packaging on Linux x64 and Windows x64. The distributed
Linux binary was built separately using the retained Ubuntu 24.04 cache, with
the same application source and feature set, and passed the ABI guard.

An isolated Ubuntu 24.04 runtime rendered the packaged Linux table and SQL
result. A synthetic 100,000-row Hive folder opened with 500-row paging, and SQL
returned the complete row count. Per-user installation, upgrade from alpha.1 and
removal preserved file defaults and saved preferences. The Arch package uses the
same binary and was checked with an isolated pacman installation and removal.

[Windows package validation](https://github.com/EtienneT/tablelamp/actions/runs/38637678052)
verified the exact portable ZIP and setup checksums, then installed alpha.1 and
upgraded to Tablelamp on a clean native Windows runner. Installed startup,
product branding, quoted file-open commands, all supported Open With entries,
default and preference preservation, and uninstaller cleanup passed. Manual
Windows desktop and physical mixed-DPI checks remain pending. Package scans
found no private builder paths or test-environment names.

Public records describe supported platforms, tested behavior and outstanding
coverage. Keep hostnames, addresses, account names, machine paths, private logs,
remote-access setup and test-session details outside the repository and releases.
Use synthetic fixtures for tests and screenshots.

## Release checklist

Run these against the extracted/installed package on Linux x64 and Windows x64. macOS checks below apply only to future community distributions. Record the version, OS/CPU, archive checksum, and result. CI covers native compilation, core/GUI logic, and packaging; it does not substitute for desktop integration testing.

1. Launch from the desktop application entry rather than the source checkout. Confirm a usable window, readable text, picker, drag/drop, and close/quit.
2. Install/register the viewer. Verify the first-launch offer appears and declining does not change the default. Quit/reopen to verify the decline is remembered. Reopen the offer through the app menu, then choose it as the default for `.parquet` and `.parq`, then double-click a file whose name contains spaces, Unicode, and `%`. On macOS repeat while the application is already running, including during a read. Verify the requested document opens.
3. Open a wide file with more columns than fit. Scroll both ways, resize/fullscreen the window, and verify the names, types, plots, and cells stay aligned. Click the horizontal scrollbar after several seconds idle. Verify the vertical scrollbar stays visible and scrolls rows at the beginning, middle, and end of the horizontal range, with plots both shown and hidden.
4. Change page size, hide plots, sort a column, navigate, and scroll. Quit and reopen the same file. Verify preferences and its view return from the documented user preferences folder, without writing inside the app's install directory. In **Settings → Appearance**, check System, Light and Dark, including restart persistence. System should follow live OS appearance changes on Windows/macOS and Linux without Omarchy; on Omarchy it should follow live palette changes. Explicit Light/Dark choices should stay fixed when the OS changes. On macOS, check that the native window chrome matches an explicit choice and resumes OS following in System mode.
5. Load All for a small file and verify the row count/sorting. Try an oversized file and verify a memory-budget message with the previous page still usable. Confirm the plot/count scope says first 1,000 rows when sampled.
6. Click numeric, temporal, Boolean and string charts; verify column details fill the app's existing window without changing its size or fullscreen state, with meaningful axes, hover counts/percentages, and the sample/null/distinct cards. Check the small charts' hover borders leave space around their content. Return via Back and Esc, including from an already fullscreen table. Use pagination page numbers and the page field.
7. Verify Windows scaling on different-DPI monitors, and verify Cmd+O/Cmd+Q plus Finder document opening on macOS. Confirm the artifact's architecture matches the machine.
8. Upgrade the installation, then uninstall. Verify launchers/registrations are removed and unrelated file associations remain intact. User settings should remain unless explicitly deleted.

The first Linux/Windows alpha is unsigned; disclose this in its release notes and verify download checksums. Authenticode signing is a future Windows distribution improvement. A future Mac maintainer must plan native validation and Developer ID/notarization for Mac downloads. Repeat smoke checks with the exact extracted artifact before publication.

## New source and overview checks

- Open IPC/Feather v2 (compressed and uncompressed), CSV and TSV, including Unicode paths and final/empty pages. Large reads remain paged. Confirm CSV quoted newlines/CRLF, delimiter/header/encoding and all-text settings; reopen to verify preferences.
- Find a late column by name/type. Click a cell, select text, copy it, and check full nested values and the explicit oversized-value limit. File details must show schema and Parquet compression/encodings/row groups.
- Check Settings, recent files, remote sources, CSV options, SQL and shortcuts in dialogs; search, column visibility, filters, file details and cell inspection in side drawers. The table should remain visible behind each tool. Close with the button, Esc and the backdrop; the table's scroll, selection and sorting must be preserved. Tab should stay within the open tool, and Enter in a text field should not dismiss it unexpectedly. Resize to a small window and verify tool contents remain reachable.
- Build the file overview on sorted data. Verify the scope label, sampled counts and expanded chart; Back returns to the overview. Cancel a long source scan; opening another source while busy must cancel/queue correctly.
- Reopen a recent file and verify its page/scroll/sort. Clear history, close and reopen; the history stays empty.
- Cloud download: open HTTP(S), S3, Azure and GCS with existing credentials. Verify range-only traffic, timeout/cancellation, repeated-page cache reuse and changed-object rejection. Signed URLs and secrets must not appear in saved preferences/history or error messages. SSO/role profiles require exported temporary credentials.
- Validate the cloud release packages. Verify Open With registration for all local formats without replacing existing defaults on Windows and Linux. Core packages and macOS are optional source-build/community checks.

## SQL and Hive release checks

- Open a local Hive folder with matching schemas and nested `key=value` partitions through the picker, command line and drag/drop. Verify virtual integer/float/Boolean/text columns, percent-decoded values and the Hive null marker. Metadata should show original rows, file count and row groups. Pages must cross files correctly. Add/change a partition and reopen.
- Save All on a small file, then open/reopen a large folder: it must start paged. Explicit All on the folder still uses the memory guard.
- Run SELECT/WHERE/ORDER BY, grouped counts, WITH, transformed columns and an empty result. Queries must use the whole source rather than the current page or normal filters. Result row numbers and chart scope must be clear; column sorting/filtering must be disabled until Exit SQL restores the previous normal view.
- Page/hide/resize result columns, save an invalid draft, quit and reopen. Recompute the last successful query and restore its separate view; the draft must remain editable. Cancel a long query, provoke a memory-budget failure and check that the previous page remains usable with no orphan worker. Query cache files should disappear when the result closes.
- Check compressed IPC with temporal/nested columns, CSV/TSV parsing options, and cloud Parquet with a signed URL and existing provider profile. SQL may stage the entire CSV/remote source; verify cancellation and avoid private URLs in preferences/logs.
- Run these on Linux x64 and Windows x64 with the downloaded package. macOS is community tooling. Native builds and headless UI tests do not prove picker/desktop behavior.


## Tablelamp rebrand source checks — 2026-10-13

- Linux cloud development build: `Tablelamp 0.1.0-alpha.2 (cloud)`; CLI help,
  title/menu/association/error branding updated. Existing executable, desktop
  identity, MIME registrations and preference locations are retained.
- Isolated Linux preview: refreshed four theme captures, tool stills and the GIF
  with synthetic data. The approved SVG supplies website and Linux icons, logos,
  the Windows ICO and the macOS source PNG.
- Formatting and all-feature Clippy passed. All 68 workspace tests passed,
  including headless UI, association compatibility and real SQL-worker tests.
- Generated website documentation is current. Homepage and installation tabs
  rendered correctly; Linux archive and Arch installer `--check` modes downloaded
  and verified the existing alpha.1 assets through the renamed repository.
- Published alpha.1 binaries retain their original name, icon and filenames.
  Alpha.2 Tablelamp packages were not rebuilt or published in this source change.
  The Arch recipe requires a new validated archive checksum before packaging;
  Windows/macOS resource changes await native release checks.

The source-only status above describes the 2026-10-13 rebrand. The 2026-10-14
release checks at the top of this document cover the new Tablelamp downloads.
