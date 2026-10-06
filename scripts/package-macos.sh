#!/usr/bin/env bash
# Run on macOS; builds a Finder application and an archive for this Mac's CPU.
set -euo pipefail
[[ $(uname -s) == Darwin ]] || { printf 'Build the macOS package on a Mac.\n' >&2; exit 1; }
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
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
case $(uname -m) in arm64) architecture=arm64 ;; x86_64) architecture=x64 ;; *) exit 1 ;; esac
bundle="$project_root/dist/Parquet Viewer.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
install -m755 target/release/parquetviewer "$bundle/Contents/MacOS/parquetviewer"
xcrun swiftc -O -parse-as-library packaging/macos/associate.swift -o "$bundle/Contents/MacOS/parquetviewer-associate"
install -m644 packaging/macos/Info.plist "$bundle/Contents/Info.plist"
install -m644 LICENSE "$bundle/Contents/Resources/LICENSE"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $version" "$bundle/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $version" "$bundle/Contents/Info.plist"
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
