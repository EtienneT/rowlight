# Working on Rowlight

Keep the app simple and lightweight. Preserve bounded paging, original column order,
and the existing GPUI Kit / Polars architecture.

## Fast development loop

- **Use development builds while implementing changes.** Use `cargo dev -- FILE`
  for a preview that rebuilds and restarts on source saves, or `cargo dev-build`
  for one incremental cloud build. Preview preferences are isolated from the
  installed app. Make the changed UI available for feedback before release work.
- Keep the cloud feature set, target directory and four-job default stable.
  Retain the build cache. Don't run `cargo clean` or change compiler/linker flags
  as a routine speed optimization; those changes invalidate cached dependencies.
- `cargo quick-check` gives compiler feedback without linking. Don't routinely
  run it immediately before a development build that checks the same code.
- Run a relevant regression with `cargo ui-test TEST_FILTER` while iterating.
  This command also builds the application binary used by real SQL-worker tests.
  Check rendered behavior for UI changes, using an owned preview and isolated
  preferences/data. Don't alter or close the user's other app instances.
- After the change settles, run formatting, appropriate lint and tests once.
  Broaden or repeat checks when a new change, failure or unresolved risk warrants
  it. Run only one Cargo operation per host/target directory at a time.
- Validate platform-specific changes on the affected platform. For shared UI
  edits, start with the host preview and focused tests; don't automatically build
  every OS's release packages. Complete native platform/release checks before
  distribution, and state which platforms and behaviors were actually tested.
- **Optimized release builds are for requested installed-build updates, packaging,
  distribution and performance measurements.** Don't run them on each edit or
  hold development feedback until installers finish. Don't substitute a dev
  binary for the installed release app without the user's request.

## Measured costs

On 2026-10-07, this UI fix took 2.8 s to build incrementally on Linux, versus
70.7 s for the optimized release build. Only the app was recompiled in both cases.
The full cached workspace test run took 7.5 s and all-feature Clippy took 0.8 s.
Windows took 18.3 s for the dev build and 15.0 s to compile and run the focused
regression. These are observations for this change, not timing guarantees.
See [development.md](docs/development.md) for commands and build details.

GitHub Actions was disabled at the user's request to avoid paid minutes. Don't
enable or dispatch workflows without an explicit request.
