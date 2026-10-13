# Tablelamp for Arch and Omarchy

New releases use `tablelamp-bin` as a native `.pkg.tar.zst` package.
The published alpha.1 package retains its original `rowlight-bin` filename.
It uses the same validated cloud executable as the Linux archive. Pacman manages
its files and dependencies; it never runs the per-user installer or writes file
default choices. Preferences retain their existing `parquetviewer` location.

An AUR listing is not yet published. The alpha.2 recipe requires the validated Tablelamp archive checksum before it
can be built or submitted. It deliberately refuses the old archive checksum.
Once the checksum is pinned:

```sh
cd packaging/arch
makepkg
makepkg --printsrcinfo > .SRCINFO
```

`tablelamp-bin` downloads a specific public release with a pinned SHA-256 checksum.
It does not require Rust. Check for an existing package before submitting a new
one, follow the [AUR submission guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines),
and maintain the recipe for future releases. Upload the recipe, `.SRCINFO` and
its source license to the AUR; do not upload the compiled package there.

For a new release, update `_version`, `pkgver`, the archive checksum and `.SRCINFO`.
Increment `pkgrel` for packaging changes without an application version change.
Test installation, file opening, opt-in associations, upgrade and removal before
publishing. Use a clean Arch environment to check dependencies independently of
the maintainer's desktop. The application remains MIT; the package recipe is 0BSD.
