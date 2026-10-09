# Install Rowlight

[User guide](user-guide.md) · [Formats](formats.md) · [Platform support](platforms.md)

Rowlight's maintained platforms are **Linux x64 and Windows x64**. The first
alpha is being prepared; published downloads will appear on the
[Releases page](https://github.com/EtienneT/rowlight/releases). You do not need
Rust to run a downloaded build.

## Choose a download

| Platform | Download | Use |
| --- | --- | --- |
| Linux x64 | `rowlight-VERSION-linux-x64-cloud.tar.gz` | Extract, then run or install |
| Windows x64 | `rowlight-VERSION-windows-x64-cloud-setup.exe` | Per-user installation |
| Windows x64 | `rowlight-VERSION-windows-x64-cloud.zip` | Portable use |

### Core or cloud

The cloud download includes local Parquet, Arrow IPC / Feather v2, CSV and TSV,
plus remote Parquet from S3, Azure, GCS and HTTP(S). Local-only core packages can
also be built from source; they omit `-cloud` in their filenames.

The alpha is unsigned. Windows may show an unrecognized-publisher warning.
Verify the download against its `.sha256` file or the release's `SHA256SUMS`:

```sh
# Linux: run beside the archive and its .sha256 file
sha256sum -c rowlight-VERSION-linux-x64-cloud.tar.gz.sha256
```

```powershell
# Windows: compare the hash with the corresponding line in SHA256SUMS
Get-FileHash .\rowlight-VERSION-windows-x64-cloud-setup.exe -Algorithm SHA256
```

> [!NOTE]
> The executable remains `parquetviewer` / `parquetviewer.exe`. Existing settings
> and application identifiers are preserved for compatibility. The visible app
> name is Rowlight; legacy Windows registrations may still say Parquet Viewer.

## Linux

Requires glibc **2.39 or later**, a graphical Wayland or X11 session, and a Vulkan
driver. Official archives are built on Ubuntu 24.04. Installation also needs
`xdg-mime`, `update-mime-database` and `update-desktop-database` (on Ubuntu,
`xdg-utils`, `shared-mime-info` and `desktop-file-utils`).

Extract the archive and open a terminal in its extracted directory:

```sh
# Run without registering anything:
./parquetviewer /path/to/file.parquet
# Or install the executable and desktop entry for your user:
./scripts/install-linux.sh
```

Installation does not require administrator privileges. Launch **Rowlight**
from the application menu or `~/.local/bin/parquetviewer`.
License notices are installed under your XDG data directory in `parquetviewer/`.

Registration preserves existing defaults. Accept the first-launch association
offer, or use **Associate Parquet files…** from the menu, to opt in to changing
Parquet's default. **No thanks** remembers your choice.

The previous default is recorded in
`${XDG_DATA_HOME:-~/.local/share}/parquetviewer/previous-default`. Restore it with
`xdg-mime default PREVIOUS.desktop application/vnd.apache.parquet application/x-parquet`,
replacing `PREVIOUS.desktop` with that saved value.

To remove the executable and registration, delete `~/.local/bin/parquetviewer`,
`applications/com.parquetviewer.ParquetViewer.desktop`,
`mime/packages/parquetviewer.xml`, and
`icons/hicolor/scalable/apps/com.parquetviewer.ParquetViewer.svg` under your XDG
data directory. Refresh the MIME/desktop databases afterwards. Keep your
preferences unless you want to reset the app.

## Windows

Requires **Windows 10 or later, x64**, with a DirectX 11 graphics driver.

Run the setup executable for a per-user installation with a Start menu entry and
**Open with** registration. For portable use, extract the ZIP and run
`parquetviewer.exe`; it does not register itself or add a Start menu entry.

To make double-click open Parquet, use **Choose in Windows Settings** in the
first-launch offer or **Associate Parquet files…** from the menu. Choose Rowlight
for `.parquet` and `.parq` in Default Apps. On Windows 10, choose defaults by file
type. You can also right-click a file → **Open with → Choose another app → Always**.
Windows owns the final default selection; installation does not replace it.

Uninstall through Windows Settings. It removes the app's registrations and keeps
preferences. On upgrades, the original `Programs\Parquet Viewer` installation
directory is retained.

## Try from source

Install Rust with rustup and the [native prerequisites](development.md#toolchain-and-checks).
The checkout pins Rust **1.99.0** and Cargo.lock. Use the fast development preview:

```sh
cargo dev -- /path/to/file.parquet
```

Source saves rebuild and restart the preview. Its settings are separate from the
installed app, and it skips the association prompt. Close the preview or press
Ctrl+C to stop. Quote paths containing spaces.

For packages, use `./scripts/package-linux-container.sh --cloud` on Linux with
Docker, or `./scripts/package-windows.ps1 -Cloud` in a native Windows MSVC/SDK
environment with Inno Setup 6. The Linux container has a separate retained cache;
it does not replace the host development cache. See [release preparation](releasing.md).

## macOS

macOS is **experimental community tooling**, outside the official release matrix.
There are no official Mac downloads or native test guarantees. Existing source
and native `scripts/package-macos.sh` / `scripts/install-macos.sh` tooling remain
available for contributors with Mac hardware. Read [platform support](platforms.md)
before relying on them.

Preferences remain in the existing `parquetviewer` user configuration directory;
see [saved settings](user-guide.md#make-the-view-your-own).
