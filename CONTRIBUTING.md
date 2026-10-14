# Contributing to Tablelamp

Tablelamp does one job: help people understand a tabular file without changing it.
Small fixes, clearer errors, accessibility improvements and format compatibility
are welcome. Discuss larger features in an issue before implementing them.

## Develop

1. Install Rust 1.99.0 and the [native prerequisites](docs/development.md).
2. Run `cargo dev -- /path/to/synthetic.parquet`. Source saves rebuild and restart
   an isolated preview; your installed app's settings stay separate.
3. Run the relevant regression with `cargo ui-test TEST_FILTER`.
4. Before submitting, run `cargo fmt --all --check`, the appropriate Clippy check
   and tests described in [development.md](docs/development.md#toolchain-and-checks).

Use development builds while iterating. Keep the four-job default and build cache;
optimized release builds belong to packaging and performance measurements.
See [AGENTS.md](AGENTS.md) for the repository's development conventions.

## Continuous integration

Native CI is configured to check Linux x64 and Windows x64 on code and build-input
changes pushed to `main`, and on non-draft pull requests targeting `main`.
Documentation, website and repository housekeeping changes skip native builds.
Marking a code-changing draft PR ready for review starts its checks; newer commits
cancel older runs. Checks use development builds and cached dependencies.

Optimized release packages are built only through a manual workflow run with the
`packages` option selected. Native jobs use standard GitHub-hosted runners and
skip private repositories.

## Website and documentation

GitHub Pages publishes the checked-in `docs/` folder whenever changes reach
`main`. Website deployment runs independently of native CI, so documentation
and website changes still publish while skipping native builds.

The four website guide pages are generated from their Markdown sources. After
editing those sources, the website template or shared website assets, run
`uv run --script scripts/build-website-docs.py` and include the generated HTML
in your commit. Use `uv run --script scripts/build-website-docs.py --check` to
verify that the generated pages are current.

## Keep changes focused

- Preserve bounded paging, original column order and read-only source access.
- Keep GPUI Kit and Polars as the UI and data engines.
- State whether statistics describe a page, sample or complete source.
- Include a small synthetic reproducer for data bugs. Do not submit private data,
  credentials, signed URLs or screenshots containing sensitive values.
- Explain the problem, the resulting behavior and what you actually tested.

Linux x64 and Windows x64 are maintained. macOS support is experimental and needs
a community maintainer with Mac hardware; see [platform support](docs/platforms.md).
A macOS contribution should include native build and desktop validation evidence.

Be considerate in issues and reviews. Critique the change, not the person.
Contributions are licensed under the project's MIT license.
