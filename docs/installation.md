# Install Tablelamp

Choose your operating system, copy one command, and open the app. The installer
downloads **Tablelamp 0.1.0-alpha.2** and checks its SHA-256 hash before running it.
No Rust installation is needed.

After installing, [download the demo dataset](https://etiennet.github.io/tablelamp/demo/demo.parquet)
and open it in Tablelamp. [Try the sample queries](demo/README.md).

## Linux

Paste this into your terminal:

```sh
curl -fsSL https://etiennet.github.io/tablelamp/install.sh | bash
```

When it finishes, open the app from your application menu. On Omarchy / Arch,
the command uses **pacman**, which asks for your password and confirmation. On
other Linux desktops, it installs for your user account without sudo. An existing
per-user Tablelamp installation is updated in place.

**Requirements:** Linux x64, glibc 2.39+ (Ubuntu 24.04 or newer), Wayland or X11,
and a working Vulkan graphics driver.

[Review the install script](https://github.com/EtienneT/tablelamp/blob/main/docs/install.sh)
· [Download an archive instead](https://github.com/EtienneT/tablelamp/releases/download/v0.1.0-alpha.2/tablelamp-0.1.0-alpha.2-linux-x64-cloud.tar.gz)

<details>
<summary>Manual installation</summary>

### Omarchy / Arch

Download the [Arch package](https://github.com/EtienneT/tablelamp/releases/download/v0.1.0-alpha.2/tablelamp-bin-0.1.0.alpha.2-1-x86_64.pkg.tar.zst)
and verify its checksum, then run this in the download folder:

```sh
sudo pacman -U ./tablelamp-bin-0.1.0.alpha.2-1-x86_64.pkg.tar.zst
```

Pacman installs Tablelamp and resolves its declared dependencies. A Vulkan driver
for your GPU is still required. The package adds Open With support and preserves
existing file defaults. Follow Omarchy's normal process for system updates.

An AUR listing is not available yet; `yay -S tablelamp-bin` is not an installation
option. If you are switching from a per-user install to the Arch package, first
run `./scripts/uninstall-linux.sh` from an extracted Tablelamp archive. This avoids
an older launcher or executable under `~/.local` taking precedence. Preferences
are kept.

### Other Linux

Download the [Linux archive](https://github.com/EtienneT/tablelamp/releases/download/v0.1.0-alpha.2/tablelamp-0.1.0-alpha.2-linux-x64-cloud.tar.gz)
and verify its checksum. Extract it, then run this from the extracted folder:

```sh
./scripts/install-linux.sh
```

The per-user installer needs `xdg-mime`, `update-mime-database`, and
`update-desktop-database`. If they are missing on Ubuntu, install them first:

```sh
sudo apt install xdg-utils shared-mime-info desktop-file-utils
```

For portable use, skip installation and run `./parquetviewer` from the extracted
folder. Keep the extracted files together.

</details>

<details>
<summary>Updates, removal, and file defaults</summary>

Rerun the quick-install command to install the version shown on this page. A new
release will have an updated command script. Your preferences and existing file
defaults are preserved.

For an Arch installation, remove the package with:

```sh
sudo pacman -R tablelamp-bin
```

For a per-user installation, download and extract the Linux archive, then run:

```sh
./scripts/uninstall-linux.sh
```

Preferences are kept. The per-user uninstaller restores an older recorded Parquet
default when Tablelamp is still the default; other default choices are left alone.

To make Tablelamp the default for Parquet, accept its first-launch offer or choose
**Associate Parquet files…** from the app menu. **No thanks** remembers your choice.
The executable remains `parquetviewer`, and settings keep their existing identity.

</details>

## Windows

Open **PowerShell** from Start and paste this command:

```powershell
irm https://etiennet.github.io/tablelamp/install.ps1 | iex
```

The command downloads and checks the installer, then opens the setup wizard.
Follow its steps and open the app from Start. It installs for your account
without an administrator prompt.

**Requirements:** Windows 10 or later, x64, with a DirectX 11 graphics driver.
This alpha is unsigned; Windows may show an unknown-publisher warning or block
the installer under your security policy.

[Review the install script](https://github.com/EtienneT/tablelamp/blob/main/docs/install.ps1)
· [Download the installer instead](https://github.com/EtienneT/tablelamp/releases/download/v0.1.0-alpha.2/tablelamp-0.1.0-alpha.2-windows-x64-cloud-setup.exe)

<details>
<summary>Portable use, updates, and removal</summary>

For portable use, download the [ZIP](https://github.com/EtienneT/tablelamp/releases/download/v0.1.0-alpha.2/tablelamp-0.1.0-alpha.2-windows-x64-cloud.zip),
verify its checksum, choose **Extract All**, and run `parquetviewer.exe`. Keep the
extracted files together. The ZIP does not add a Start menu entry.

To update an installation, rerun the quick-install command when this page points
to a newer release, or run that release's setup executable. For a portable update,
close Tablelamp and extract the newer ZIP into a new folder. Preferences are kept.

Remove the installed app through **Settings → Apps → Installed apps → Tablelamp →
Uninstall** (**Apps & features** on Windows 10). It removes registrations and
keeps preferences. Upgrades retain the original `Programs\Parquet Viewer` directory.

</details>

<details>
<summary>Make Tablelamp the default for Parquet</summary>

Use **Choose in Windows Settings** in the first-launch offer, or **Associate
Parquet files…** from the app menu. Choose Tablelamp for `.parquet` and `.parq` in
Default Apps. You can also right-click a file → **Open with → Choose another app
→ Always**. Windows owns the final default selection; installation preserves it.

</details>

## Download checksums

The quick-install commands verify the downloaded package against the release's
[SHA256SUMS](https://github.com/EtienneT/tablelamp/releases/download/v0.1.0-alpha.2/SHA256SUMS).
Checksums verify file integrity, not publisher identity. This alpha is unsigned.

<details>
<summary>Verify a manually downloaded file</summary>

Download `SHA256SUMS` from the same release as your package. On Linux, run this
beside the package and checksum file:

```sh
sha256sum --ignore-missing --check SHA256SUMS
```

On Windows, compare this output with the matching line in `SHA256SUMS`:

```powershell
Get-FileHash .\tablelamp-0.1.0-alpha.2-windows-x64-cloud-setup.exe -Algorithm SHA256
```

Expect `OK` on Linux or an identical hash on Windows. If verification fails,
download again before installing. GitHub's **Source code** archives are for
building the project; use the packaged downloads to install Tablelamp.

</details>

## Included in the download

<span id="core-or-cloud"></span>

The standard download includes local Parquet, Arrow IPC / Feather v2, CSV and TSV,
plus remote Parquet from S3, Azure, GCS and HTTP(S). It uses existing provider
credentials. A local-only core build can be made from source.

## Try from source

Install Rust and the [native prerequisites](development.md#toolchain-and-checks),
then run this from a checkout:

```sh
cargo dev -- /path/to/file.parquet
```

Source saves rebuild and restart the preview. Its settings are isolated from the
installed app. Close the preview or press Ctrl+C to stop. See
[development](development.md) and [release preparation](releasing.md) for build details.

## macOS

macOS is experimental community tooling, with no official downloads or
quick-install command. Contributors can use the existing source and native
packaging tools. Read [platform support](platforms.md) before relying on them.

[Start using Tablelamp](user-guide.md) · [Supported formats](formats.md)
· [Keyboard shortcuts](keyboard-shortcuts.md)
