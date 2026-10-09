# Install Rowlight

[User guide](user-guide.md) · [Formats](formats.md) · [Platform support](platforms.md)

Rowlight's maintained platforms are **Linux x64 and Windows x64**. The first
alpha is being prepared; downloads are not published yet. When available, open
the [Releases page](https://github.com/EtienneT/rowlight/releases), select a release
and expand **Assets**. Alpha releases are marked **Pre-release**. You do not need
Rust to run a downloaded build. GitHub's automatic **Source code** archives are
for building the project, not installing the application.

## Choose a download

| Platform | Download | Use |
| --- | --- | --- |
| Linux x64 | `rowlight-VERSION-linux-x64-cloud.tar.gz` | Extract, then run or install |
| Windows x64 | `rowlight-VERSION-windows-x64-cloud-setup.exe` | Recommended; per-user installation |
| Windows x64 | `rowlight-VERSION-windows-x64-cloud.zip` | Portable use |

`VERSION` is the release version, for example `0.1.0-alpha.1`. Download the
matching `SHA256SUMS` file as well. Installation steps are below:
[Linux](#linux) · [Windows](#windows).

### Core or cloud

The cloud download includes local Parquet, Arrow IPC / Feather v2, CSV and TSV,
plus remote Parquet from S3, Azure, GCS and HTTP(S). Local-only core packages can
also be built from source; they omit `-cloud` in their filenames.

The alpha is unsigned. Windows may show an unrecognized-publisher warning.
Verify the download against the release's `SHA256SUMS`:

```sh
# Linux: run beside the downloaded archive and SHA256SUMS
sha256sum --ignore-missing --check SHA256SUMS
```

```powershell
# Windows: compare the hash with the corresponding line in SHA256SUMS
Get-FileHash .\rowlight-VERSION-windows-x64-cloud-setup.exe -Algorithm SHA256
```

Expect `OK` on Linux or an identical hash on Windows. If verification fails,
download the file again before installing. Checksums detect damaged or changed
downloads; they do not verify the publisher's identity. See Microsoft's
[signing guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).

> [!NOTE]
> The executable remains `parquetviewer` / `parquetviewer.exe`. Existing settings
> and application identifiers are preserved for compatibility. The visible app
> name is Rowlight; legacy Windows registrations may still say Parquet Viewer.

## Linux

Requires glibc **2.39 or later**, a graphical Wayland or X11 session, and a Vulkan
driver. Official archives are built on Ubuntu 24.04. Installation also needs
`xdg-mime`, `update-mime-database` and `update-desktop-database` (on Ubuntu,
`xdg-utils`, `shared-mime-info` and `desktop-file-utils`).

1. Download the Linux archive and verify its checksum.
2. Extract it using your file manager, or run the following in your Downloads
   folder, replacing `VERSION` with the downloaded version:

   ```sh
   tar -xzf rowlight-VERSION-linux-x64-cloud.tar.gz
   cd rowlight-VERSION-linux-x64-cloud
   ./scripts/install-linux.sh
   ```

3. Open **Rowlight** from the application menu, then choose **Open file**.

Installation does not require administrator privileges. Launch **Rowlight**
from the application menu or `~/.local/bin/parquetviewer`.
License notices are installed under your XDG data directory in `parquetviewer/`.

For portable use, skip the installer and run `./parquetviewer` from the extracted
folder. To open a file directly, run `./parquetviewer "/path/to/file.parquet"`.
To update an installation, extract the new release and run its installer again.
Your preferences and existing file defaults are preserved.

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

1. Download `rowlight-VERSION-windows-x64-cloud-setup.exe` and verify its checksum.
2. Double-click the installer and follow its steps. It installs for your Windows
   account without an administrator prompt, adds **Rowlight** to Start, and
   registers it under **Open with**.
3. Open **Rowlight** from Start, then choose **Open file**.

The first alpha is unsigned. Windows security settings may warn about an unknown
publisher or block it; a checksum does not replace a trusted signature.

For portable use, right-click the ZIP, choose **Extract All**, and run
`parquetviewer.exe` from the extracted folder. Keep the extracted files together;
the ZIP does not include an installer or add a Start menu entry.

To update an installation, run the newer setup executable. It retains your
preferences and existing file defaults. For a portable update, close the app and
extract the new ZIP into a new folder.

To make double-click open Parquet, use **Choose in Windows Settings** in the
first-launch offer or **Associate Parquet files…** from the menu. Choose Rowlight
for `.parquet` and `.parq` in Default Apps. On Windows 10, choose defaults by file
type. You can also right-click a file → **Open with → Choose another app → Always**.
Windows owns the final default selection; installation does not replace it.

Uninstall through **Settings → Apps → Installed apps → Rowlight → Uninstall**
(**Apps & features** on Windows 10). It removes the app's registrations and keeps
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
