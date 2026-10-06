# Install Rowlight

[User guide](user-guide.md) · [Formats](formats.md) · [Keyboard shortcuts](keyboard-shortcuts.md)

Choose your platform: [Linux](#linux) · [Windows](#windows) · [macOS](#macos). To run from source without installing, use [Try from source](#try-from-source).

> [!NOTE]
> Rowlight's executable, application bundle, package filenames, and preferences directory currently retain the original **Parquet Viewer / `parquetviewer`** name. Use those names in the commands and file-association dialogs below.

## Get a build

The [Native builds workflow](https://github.com/EtienneT/rowlight/actions/workflows/ci.yml) targets Linux x64, Windows x64, and macOS on Apple Silicon and Intel. Download an artifact from a successful run for your platform, if available, or build from source below. GitHub access to this repository is required to download its workflow artifacts.

These are development packages. Publicly signed releases are not yet prepared: the macOS bundle uses an ad hoc signature, and the Windows installer is unsigned. Native Windows/macOS desktop release checks remain pending; see [validation status](#validation-status).

### Core or cloud

**Core** includes local Parquet, Arrow IPC / Feather v2, CSV, and TSV. **Cloud** includes those formats plus remote Parquet from S3, Azure, GCS, and HTTP(S). The cloud package has `-cloud` in its filename; both variants share the same preferences and application identity.

For source packaging, add `--cloud` to the Linux/macOS scripts, or `-Cloud` to the Windows script. For an uninstalled source run, add `--features cloud` to Cargo. See [remote Parquet setup](formats.md#remote-parquet).

## Linux

Use a graphical Wayland or X11 session with a working Vulkan driver. The installer needs `xdg-mime`, `update-mime-database`, and `update-desktop-database`.

### Install a downloaded archive

1. Extract the Linux archive and open a terminal in its `parquetviewer` directory.
2. Run:

   ```sh
   ./scripts/install-linux.sh
   ```

3. Launch **Parquet Viewer** from the application menu, or open `~/.local/bin/parquetviewer`.

The archive contains its executable, so no Rust toolchain is needed. Installation is per user and does not require administrator privileges.

### Build and install from source

Install Rust and the [native GPUI prerequisites](https://gpui-kit.com/docs/installation/), then run from the checkout:

```sh
./scripts/install-linux.sh
# Include remote Parquet support:
./scripts/install-linux.sh --cloud
```

The script builds the release executable and installs it in `~/.local/bin`. Use `--skip-build` only when the desired release executable is already available. To create an archive without installing, run `./scripts/package-linux.sh` (optionally with `--cloud`).

### Make double-click open Parquet

On first launch, accept the association offer, or choose **Associate Parquet files…** from the top-left menu later. **No thanks** remembers your decision. Registration preserves existing defaults for the advertised formats until you opt in to changing Parquet's default.

The installer records the previous Parquet default in `${XDG_DATA_HOME:-~/.local/share}/parquetviewer/previous-default`. To restore it, read that file and replace `PREVIOUS.desktop` below with the saved application:

```sh
xdg-mime default PREVIOUS.desktop application/vnd.apache.parquet application/x-parquet
```

The launcher has an absolute executable path, so double-click opening does not depend on the file manager's `PATH`. Installed registrations and icons live under your XDG data directory; the executable lives under `~/.local/bin`.

## Windows

Requires **Windows 10 or later, x64**, with a DirectX 11 graphics driver.

### Install or run a downloaded build

Run `parquetviewer-windows-x64-setup.exe` for a per-user installation with a Start menu entry and **Open with** registration. A cloud build is named `parquetviewer-windows-x64-cloud-setup.exe`.

For portable use, extract the ZIP and run `parquetviewer.exe`. Use the picker, drag/drop, or a command-line path. The portable build does not register itself or add a Start menu entry.

### Make double-click open Parquet

1. Use the first-launch **Choose in Windows Settings** button, or **Associate Parquet files…** from the app menu.
2. In Default Apps, choose **Parquet Viewer** for `.parquet` / `.parq`. On Windows 10, use **Choose default apps by file type**.
3. Alternatively, right-click a file → **Open with → Choose another app → Always**.

Windows owns the final default-app selection. The installer registers the viewer for supported local formats without overwriting that selection. Uninstall removes the app and its registrations but keeps your preferences.

### Build from source

Install Rust's **MSVC** toolchain, Visual Studio C++ Build Tools with a Windows SDK, CMake, and [Inno Setup 6](https://jrsoftware.org/isinfo.php). Run from the checkout in PowerShell:

```powershell
./scripts/package-windows.ps1
# Include remote Parquet support:
./scripts/package-windows.ps1 -Cloud
```

This produces the setup executable and portable ZIP under `dist/`. The package uses a static C runtime, embeds its icon/version information, and uses GPUI's per-monitor DPI manifest.

## macOS

Requires **macOS 15 or later** on Apple Silicon or Intel. Choose the archive matching your Mac's CPU (`arm64` or `x64`).

### Install a downloaded build

Extract the ZIP, then move **Parquet Viewer.app** to Applications and open it. Rust and Swift are not needed to run the bundled app.

### Make double-click open Parquet

Choose **Make default** in the first-launch offer, or reopen it through **Associate Parquet files…**. Declining preserves your existing default. Finder also offers **Get Info → Open with → Change All**.

The bundle declares supported local formats and handles Finder document-open events at launch or while running. Files opened during a read are queued and displayed afterward.

### Build from source

Install Rust and Xcode Command Line Tools, following the [native GPUI prerequisites](https://gpui-kit.com/docs/installation/). Build natively on the Mac you are targeting:

```sh
./scripts/package-macos.sh
# Or build, install to ~/Applications, and register with Finder:
./scripts/install-macos.sh
```

Add `--cloud` for remote Parquet support. Packaging creates `dist/Parquet Viewer.app` and a ZIP for the current architecture. The default-association helper is bundled; the user does not need to compile it.

## Try from source

The checkout pins **Rust 1.99.0** and its application lockfile. Install Rust with rustup and the [platform prerequisites](https://gpui-kit.com/docs/installation/), then run from the project root:

```sh
cargo dev -- /path/to/file.parquet
```

This opens a development preview with cloud support, watches source changes, and saves preview preferences separately from the installed app. It skips the association prompt. Close the preview or press Ctrl+C to stop. Quote paths containing spaces.

For a normal release run without installation:

```sh
cargo run --locked --release -- /path/to/file.parquet
# With remote support:
cargo run --locked --release --features cloud -- 's3://my-bucket/data/example.parquet'
```

The first build compiles substantial native dependencies. For subsequent changes, use the [fast development loop](development.md) instead of rebuilding release every time.

## Validation status

Linux has been built and exercised in a native window. The workflow is configured to build, test, and package all four platform/CPU targets; inspect its [current results](https://github.com/EtienneT/rowlight/actions/workflows/ci.yml) before relying on an artifact.

Before distributing Windows/macOS packages, complete the [native desktop release checks](../packaging/TESTING.md), including file associations, scaling, and both Mac architectures. Public distribution additionally needs Windows application/installer signing and macOS Developer ID signing/notarization. Source builds require no project signing credentials.
