# Rowlight media

The README uses a vector logo, four captures of the running Linux application, and a short recording of its real controls. All displayed values come from the repeatable 50,000-row synthetic fixture in `examples/sample.rs`.

## Logo

Three horizontal rows form the mark. The middle row lights up in warm amber, ending in a small point of light. The shape stays simple at icon sizes and connects directly to **Shed light on your data.**

- [Standalone mark](assets/icon.svg)
- [Wordmark on a light background](assets/logo-light.svg)
- [Wordmark on a dark background](assets/logo-dark.svg)

The README selects the wordmark for GitHub's light or dark appearance. These are original, editable SVG assets; the project's MIT license also covers them.

## Screenshots and demo

The theme gallery shows **Tokyo Night**, **Catppuccin**, **Gruvbox**, and **Flexoki Light**, using the stock Omarchy `colors.toml` palettes through gpui-omarchy. The same file, column widths, page size, and window size are used for every capture. These screenshots show Linux theme integration; they do not establish Windows or macOS runtime validation.

The demo shows the table and distributions, an expanded numeric chart with statistics and a tooltip, cell inspection, next-page navigation, column search, and collapsed plot headers. The column-search and cell-inspector stills show the current side drawers; the older recording shows these tools as separate views. Additional stills are kept for documentation:

- [Expanded column chart](assets/column-detail.png)
- [Cell inspector](assets/cell-inspector.png)
- [Column search](assets/find-column.png)
- [Compact table](assets/compact-table.png)
- [SQL dialog](assets/sql-query.png): optimized Linux app in its built-in Dark appearance, querying a synthetic Hive folder containing two copies of the fixture under `year=2025/site=North` and `year=2026/site=South`. Captured on 2026-10-07 using an isolated display and preferences.

The GIF is scaled to 1,000 pixels wide and uses eight frames per second with a bounded color palette. The source recording and temporary data remain under ignored `target/readme-preview/`.

## Reproduce

On Linux, supply Xvfb, xdotool, ImageMagick, FFmpeg, the native GPUI prerequisites, and an Omarchy theme directory:

```sh
python3 scripts/capture-readme.py
# Tools or palettes outside the usual locations:
python3 scripts/capture-readme.py \
  --xvfb /path/to/Xvfb \
  --xdotool /path/to/xdotool \
  --themes-dir /path/to/omarchy/themes
```

The script uses `cargo dev-build` and the existing cloud development feature set, reusing the normal incremental cache. It opens a private X11 display and sets `PARQUETVIEWER_SETTINGS_PATH` for its child app. `ROWLIGHT_PREVIEW_THEME` points to a copied Omarchy `current` directory; this preview override is compiled only in debug builds. Ordinary app startup follows the desktop theme.

The capture does not change the desktop theme, file associations, or production preferences. It owns and closes its preview/recording/display processes, including when a capture fails. Its pointer coordinates describe the fixed 1,400 × 860 capture layout and may need adjustment after UI layout changes.

For the README structure, the references were [Lapce](https://github.com/lapce/lapce) and [Zed](https://github.com/zed-industries/zed): introduce the application briefly and link to detailed documentation.
