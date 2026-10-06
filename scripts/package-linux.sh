#!/usr/bin/env bash
# Downloadable archive with a user installer and desktop/MIME/icon registration.
set -euo pipefail
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || { printf 'Build this package on Linux x64.\n' >&2; exit 1; }
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$project_root"
skip_build=false
cloud=false
for argument in "$@"; do
  case "$argument" in --skip-build) skip_build=true ;; --cloud) cloud=true ;; *) printf 'Unknown option: %s\n' "$argument" >&2; exit 1 ;; esac
done
features=()
suffix=""
if $cloud; then features=(--features cloud); suffix="-cloud"; fi
if ! $skip_build; then cargo build --locked --release --bin parquetviewer "${features[@]}"; fi
package="$project_root/dist/parquetviewer$suffix"
mkdir -p "$package/scripts" "$package/packaging/linux" "$package/docs"
install -m755 target/release/parquetviewer "$package/parquetviewer"
install -m755 scripts/install-linux.sh "$package/scripts/install-linux.sh"
install -m644 packaging/linux/* "$package/packaging/linux/"
install -m644 LICENSE README.md "$package/"
install -m644 docs/*.md "$package/docs/"
install -m644 packaging/TESTING.md "$package/packaging/"
tar -czf dist/parquetviewer-linux-x64$suffix.tar.gz -C dist "parquetviewer$suffix"
printf 'Created %s\n' "$project_root/dist/parquetviewer-linux-x64$suffix.tar.gz"
