# Prepare a release

Maintained downloads are Linux x64 and Windows x64, with cloud support included.
macOS source tooling is experimental. GitHub Actions remains disabled; do not
enable or dispatch it without the owner's explicit request.

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
7. Collect validated native downloads:

   ```sh
   python3 scripts/prepare-release.py
   ```

   `dist/release-VERSION/` contains the three downloads, SHA256SUMS, release notes
   and source provenance. It is a **local draft**, not a published GitHub release.
   The manifest explicitly records uncommitted source; never tag that draft as
   though it came from the older base commit. After committing, regenerate it.

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

Create a tag at the reviewed commit, use the local release notes, attach the three
validated downloads and SHA256SUMS, and mark the release as a prerelease. A GitHub
draft can be reviewed before publication. Do not enable Actions as a side effect.
Replace the README's alpha-preparation sentence with the actual release link.

For the first alpha, signing is deliberately deferred and the release notes
disclose it. Checksums do not replace Authenticode. Mac signing/notarization is
outside the current release scope.
