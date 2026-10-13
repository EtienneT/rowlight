# Prepare a release

Maintained downloads are Linux x64 and Windows x64, with cloud support included.
macOS source tooling is experimental. GitHub Actions remains disabled; do not
enable or dispatch it without the owner's explicit request.

## Private preparation, public downloads

Build, test and prepare the release while the repository is private. A GitHub
draft can hold the notes and downloads for review before publication. Private
repository downloads require repository access; they are not a public install
channel.

For the first public release, complete the history/license review and remaining
[native release checks](../packaging/TESTING.md), make the repository public, then
publish **v0.1.0-alpha.1** as a prerelease at the reviewed commit. Upload all four
packages and `SHA256SUMS` before publishing. Update the
README's installation section to link to that release. Creating releases and
uploading locally built packages does not require GitHub Actions.

GitHub documents [draft and prerelease publication](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository).
If immutable releases are enabled later, draft first: assets cannot be replaced
after publication. Publish a new version for corrected binaries.

## Build and validate

1. Set the version in Cargo.toml, update only the root package entry in Cargo.lock,
   and write the changelog/release notes. Use a prerelease version for an alpha.
2. Regenerate and review [third-party notices](licenses.md). Package scripts
   refuse notices whose Cargo.lock hash is stale.
3. Complete development-profile lint/tests and an isolated native UI preview.
4. Build the Linux download on the Ubuntu 24.04 baseline:

   ```sh
   ./scripts/package-linux-container.sh --cloud
   ```

   This uses a separate persistent target directory with four jobs. Do not copy
   the host development/release objects into this cache. Newer hosts can link
   against newer glibc symbols; `check-linux-abi.sh` rejects requirements above
   2.39. The archive is not a statically linked or universal Linux binary.

5. On native Windows with MSVC, a Windows SDK and Inno Setup 6:

   ```powershell
   ./scripts/package-windows.ps1 -Cloud
   ```

   Keep its release cache. The static C runtime is included; GPUI provides the
   Windows manifest. The script waits explicitly for GUI-subsystem version checks.

6. Run [native release checks](../packaging/TESTING.md) on extracted packages,
   including install/uninstall/default preservation. Record actual results and
   outstanding checks. Linux can additionally use `packaging/linux/Smoke.Dockerfile`
   for an Ubuntu runtime environment without Rust or development libraries.
7. Build the Arch package from that Linux archive. Set its SHA-256 checksum in
   `packaging/arch/PKGBUILD`, build with `makepkg`, generate `.SRCINFO` with
   `makepkg --printsrcinfo`, and copy the package and its `.sha256` file to `dist/`.
   For a local build before the GitHub release exists, put the matching archive
   in the recipe's working directory. Makepkg verifies its pinned checksum.
   Validate installation and removal in an isolated Arch root or clean system.
8. Collect validated native downloads:

   ```sh
   python3 scripts/prepare-release.py
   ```

   `dist/release-VERSION/` contains the four downloads, individual checksums,
   SHA256SUMS, release notes
   and source provenance. It is a **local draft**, not a published GitHub release.
   The manifest explicitly records uncommitted source; never tag that draft as
   though it came from the older base commit. After committing, regenerate it.
   If reusing a native Windows build after Linux-only changes, record its source
   commit using `--windows-build-commit COMMIT` and document that reuse in the
   validation record. Rebuild Windows when its code or packaging changes.

## Before making the repository public

- Review the entire Git history for secrets, private data and internal paths,
  including README media and commit authorship. Scan the final source as well.
- Check the license inventory and synthetic examples. Do not include test logs,
  credentials, private fixtures or the target directory in downloads.
- Check the public README, contributor guide, issue template and platform policy.
- Obtain approval to commit/push the prepared changes and change repository
  visibility. Making a repository public exposes its full Git history.
- When public, enable GitHub private vulnerability reporting and check its report
  link. This does not require enabling Actions.

## Publish after approval

Create a tag at the reviewed commit, use the local release notes, attach the four
validated downloads and SHA256SUMS, and mark the release as a prerelease. A GitHub
draft can be reviewed before publication. Do not enable Actions as a side effect.
Replace the README's alpha-preparation sentence with the actual release link.

## Distribution policy

- **Linux:** offer the native Arch package for Omarchy/Arch and the archive and
  per-user installer for other distributions, with the
  glibc baseline visible beside the download. Preserve desktop registration,
  preferences and file defaults on upgrades. A Flatpak can be evaluated later
  for broader distro coverage; the current archive is not universal.
  Publish the AUR recipe when an account is available; until then, users install
  the downloadable Arch package with `pacman -U` and update it manually.
- **Windows:** recommend the per-user setup executable, with the portable ZIP as
  an alternative. Preserve the stable installer/application identity on upgrades
  and let users choose file defaults through Windows.
- **Every release:** use a versioned tag, include license notices and SHA256SUMS,
  document requirements and known limitations, and validate the exact downloads.
  Keep alpha/beta versions marked as prereleases. Package-manager instructions
  should be added only when an actual package is available.

The first alpha is currently unsigned; disclose that beside the installation
steps and in the release notes. For wider Windows distribution, plan Authenticode
signing of both the application and installer, with timestamping, before computing
the final package checksums. Signing identifies the publisher but new releases
can still trigger [SmartScreen warnings](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation).
Microsoft's [signing options](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options)
include the [SignPath Foundation](https://signpath.org/terms), which offers free
signing for qualifying open-source projects. Its conditions include an existing
release and a verifiable build/reputation; approval is not guaranteed. Evaluate
it after the initial alpha rather than assuming it can sign the first release.

When public, standard GitHub-hosted runners are
[free](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
[Larger runners are billed](https://docs.github.com/en/billing/reference/actions-runner-pricing).
This does not change the owner's decision to keep Actions disabled.

Mac signing/notarization is outside the current release scope.
