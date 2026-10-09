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
binary="${CARGO_TARGET_DIR:-$project_root/target}/release/parquetviewer"
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "parquetviewer"))')
variant=core
if $cloud; then variant=cloud; fi
[[ $("$binary" --version) == "Rowlight $version ($variant)" ]] || { printf 'Release executable version or feature set does not match.\n' >&2; exit 1; }
./scripts/check-linux-abi.sh "$binary"
[[ $(sha256sum Cargo.lock | cut -d' ' -f1) == "$(cat packaging/licenses.lock.sha256)" ]] || { printf 'Regenerate third-party notices after changing Cargo.lock.\n' >&2; exit 1; }
name="rowlight-$version-linux-x64$suffix"
package="$project_root/dist/$name"
rm -rf -- "$package"
mkdir -p "$package/scripts" "$package/packaging/linux" "$package/docs/assets"
install -m755 "$binary" "$package/parquetviewer"
install -m755 scripts/install-linux.sh "$package/scripts/install-linux.sh"
install -m644 packaging/linux/* "$package/packaging/linux/"
install -m644 LICENSE README.md THIRD_PARTY_NOTICES.html "$package/"
install -m644 docs/*.md "$package/docs/"
install -m644 docs/assets/* "$package/docs/assets/"
install -m644 packaging/TESTING.md "$package/packaging/"
tar -czf "dist/$name.tar.gz" -C dist "$name"
(cd dist && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256")
printf 'Created %s\n' "$project_root/dist/$name.tar.gz"
