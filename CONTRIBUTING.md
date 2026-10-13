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
