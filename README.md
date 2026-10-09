<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/logo-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="docs/assets/logo-light.svg">
    <img src="docs/assets/logo-light.svg" alt="Rowlight" width="360" height="108">
  </picture>
</p>

<h3 align="center">Shed light on your data.</h3>

<p align="center">
  A small, read-only desktop viewer for Parquet, Arrow IPC / Feather, CSV, and TSV.<br>
  Built with Rust, GPUI Kit, and Polars. For Linux and Windows.
</p>

<p align="center">
  <a href="docs/user-guide.md">User guide</a> ·
  <a href="https://github.com/EtienneT/rowlight/releases">Downloads</a> ·
  <a href="docs/development.md">Development</a> ·
  <a href="CONTRIBUTING.md">Contribute</a> ·
  <a href="LICENSE">MIT license</a>
</p>

![Rowlight showing typed columns, distributions, and paged rows in Tokyo Night](docs/assets/tokyo-night.png)

## A quick look

![Rowlight demo: expand a chart, inspect a cell, page through rows, and collapse plots](docs/assets/rowlight-demo.gif)

Open a file, see its schema and distributions, and explore without changing the data.

- **See the shape:** column types, nulls, distributions, and expanded charts.
- **Explore:** sort the loaded page, filter or query the source with SQL, find columns, and inspect cells.
- **Keep it light:** paged reads, virtual scrolling, and guarded All mode.
- **Pick up where you left off:** saved view settings and keyboard navigation.

Plots summarize the displayed rows; **File overview** provides a bounded sample across the file. Open local Hive Parquet folders with paging across partitions. Optional cloud builds open remote Parquet from S3, Azure, GCS, and HTTP(S).

## Your Omarchy theme

Rowlight follows the current Omarchy palette, including live theme changes. These are captures of the same synthetic file in the running Linux app.

| Tokyo Night | Catppuccin |
| :---: | :---: |
| [<img src="docs/assets/tokyo-night.png" alt="Tokyo Night" width="420">](docs/assets/tokyo-night.png) | [<img src="docs/assets/catppuccin.png" alt="Catppuccin" width="420">](docs/assets/catppuccin.png) |
| **Gruvbox** | **Flexoki Light** |
| [<img src="docs/assets/gruvbox.png" alt="Gruvbox" width="420">](docs/assets/gruvbox.png) | [<img src="docs/assets/flexoki-light.png" alt="Flexoki Light" width="420">](docs/assets/flexoki-light.png) |

## Install

The first alpha is being prepared; downloads are not published yet. Once available,
open [Releases](https://github.com/EtienneT/rowlight/releases), select the release,
and expand **Assets**. No Rust installation is needed.

- **Windows 10+ x64:** download the file ending in `-windows-x64-cloud-setup.exe`,
  run it, and open **Rowlight** from Start. A portable ZIP is also available.
- **Linux x64:** download the file ending in `-linux-x64-cloud.tar.gz`, extract it,
  and run this from the extracted folder:

  ```sh
  ./scripts/install-linux.sh
  ```

  Open **Rowlight** from the application menu. Requires glibc 2.39+ (Ubuntu 24.04
  baseline), Wayland or X11, and Vulkan.

The alpha is unsigned; Windows may show a publisher warning.
[Installation, checksums, and file associations](docs/installation.md) ·
[Platform support](docs/platforms.md)

## Run from source

With Rust and the [native prerequisites](https://gpui-kit.com/docs/installation/) installed:

```sh
cargo dev -- /path/to/file.parquet
```

[Development](docs/development.md) · [Keyboard shortcuts](docs/keyboard-shortcuts.md)
