#!/usr/bin/env bash
# Experimental community tooling, not part of the maintained release matrix.
set -euo pipefail
[[ $(uname -s) == Darwin ]] || { printf 'Build the macOS package on a Mac.\n' >&2; exit 1; }
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$project_root"
skip_build=false
cloud=false
for argument in "$@"; do
  case "$argument" in --skip-build) skip_build=true ;; --cloud) cloud=true ;; *) printf 'Unknown option: %s\n' "$argument" >&2; exit 1 ;; esac
done
build_arguments=(--locked --release --bin parquetviewer)
suffix=""
# Bash 3.2 (bundled with macOS) treats an empty array as unset under `set -u`.
if $cloud; then build_arguments+=(--features cloud); suffix="-cloud"; fi
if ! $skip_build; then cargo build "${build_arguments[@]}"; fi
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
# Apple bundle versions are numeric even when Cargo uses a prerelease suffix.
bundle_version=${version%%-*}
case $(uname -m) in arm64) architecture=arm64 ;; x86_64) architecture=x64 ;; *) exit 1 ;; esac
bundle="$project_root/dist/Parquet Viewer.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
install -m755 target/release/parquetviewer "$bundle/Contents/MacOS/parquetviewer"
xcrun swiftc -O -parse-as-library packaging/macos/associate.swift -o "$bundle/Contents/MacOS/parquetviewer-associate"
install -m644 packaging/macos/Info.plist "$bundle/Contents/Info.plist"
install -m644 LICENSE "$bundle/Contents/Resources/LICENSE"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $bundle_version" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $bundle_version" "$bundle/Contents/Info.plist"
icon_directory=$(mktemp -d "$project_root/dist/icon.XXXXXX")
trap 'rm -rf "$icon_directory"' EXIT
mkdir "$icon_directory/ParquetViewer.iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" packaging/icon.png --out "$icon_directory/ParquetViewer.iconset/icon_${size}x${size}.png" >/dev/null
  doubled=$((size * 2))
  sips -z "$doubled" "$doubled" packaging/icon.png --out "$icon_directory/ParquetViewer.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$icon_directory/ParquetViewer.iconset" -o "$bundle/Contents/Resources/ParquetViewer.icns"
plutil -lint "$bundle/Contents/Info.plist"
# Local/CI seal. Public distribution additionally needs Developer ID/notarization.
codesign --force --sign - "$bundle/Contents/MacOS/parquetviewer-associate"
codesign --force --sign - "$bundle"
codesign --verify --deep --strict "$bundle"
ditto -c -k --sequesterRsrc --keepParent "$bundle" "$project_root/dist/parquetviewer-macos-$architecture$suffix.zip"
printf 'Created %s\n' "$bundle" "$project_root/dist/parquetviewer-macos-$architecture$suffix.zip"
