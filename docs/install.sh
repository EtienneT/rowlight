#!/usr/bin/env bash
# Rowlight website installer. Download a published package and verify its hash.
set -euo pipefail

version=0.1.0-alpha.1
package_version=0.1.0.alpha.1
release_url="https://github.com/EtienneT/rowlight/releases/download/v$version"
install_mode=auto
check_only=false
for argument in "$@"; do
  case "$argument" in
    --check) check_only=true ;;
    --user) install_mode=user ;;
    --arch) install_mode=arch ;;
    --help) printf 'Usage: bash install.sh [--check] [--user | --arch]\n--check downloads and verifies without installing.\n'; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$argument" >&2; exit 1 ;;
  esac
done
fail() { printf 'Rowlight: %s\n' "$*" >&2; exit 1; }
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || fail 'Official Linux downloads require Linux x64.'
if ! $check_only && [[ $EUID == 0 ]]; then fail 'Run this as your normal desktop user, without sudo.'; fi
for required in curl sha256sum mktemp getconf awk; do
  command -v "$required" >/dev/null || fail "Required command missing: $required"
done
libc=$(getconf GNU_LIBC_VERSION 2>/dev/null) || fail 'A glibc-based Linux desktop is required.'
[[ $libc == 'glibc '* ]] || fail 'A glibc-based Linux desktop is required.'
IFS=. read -r libc_major libc_minor _ <<< "${libc#glibc }"
[[ $libc_major =~ ^[0-9]+$ && $libc_minor =~ ^[0-9]+$ ]] || fail 'Could not determine the glibc version.'
(( libc_major > 2 || (libc_major == 2 && libc_minor >= 39) )) || fail 'glibc 2.39 or later is required (Ubuntu 24.04 or newer).'

if [[ $install_mode == auto ]]; then
  if command -v pacman >/dev/null && [[ ! -e "$HOME/.local/bin/parquetviewer" ]]; then install_mode=arch; else install_mode=user; fi
fi
if [[ $install_mode == arch ]]; then
  command -v pacman >/dev/null || fail 'The Arch package requires pacman; use --user for a per-user archive install.'
  if ! $check_only; then
    [[ ! -e "$HOME/.local/bin/parquetviewer" ]] || fail 'Remove your existing per-user installation before switching to the Arch package. Your preferences can be kept.'
    command -v sudo >/dev/null || fail 'sudo is required to install the Arch package.'
  fi
  asset="rowlight-bin-$package_version-1-x86_64.pkg.tar.zst"
else
  command -v tar >/dev/null || fail 'Required command missing: tar'
  if ! $check_only; then
    for required in xdg-mime update-mime-database update-desktop-database; do
      command -v "$required" >/dev/null || fail "Missing $required. On Ubuntu, install xdg-utils, shared-mime-info and desktop-file-utils, then rerun this command."
    done
  fi
  asset="rowlight-$version-linux-x64-cloud.tar.gz"
fi

stage=$(mktemp -d "${TMPDIR:-/tmp}/rowlight-install.XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT
download() { curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 "$release_url/$1" --output "$stage/$1"; }
printf 'Downloading Rowlight %s (%s install)…\n' "$version" "$install_mode"
download SHA256SUMS
download "$asset"
expected=$(awk -v asset="$asset" '$2 == asset {print $1}' "$stage/SHA256SUMS")
[[ $expected =~ ^[a-fA-F0-9]{64}$ ]] || fail 'The release does not contain one valid checksum for this download.'
actual=$(sha256sum "$stage/$asset")
[[ ${actual%% *} == "${expected,,}" ]] || fail 'Checksum verification failed. Nothing was installed.'
printf 'Download checksum verified.\n'
if [[ $install_mode == user ]]; then
  tar --extract --gzip --file "$stage/$asset" --directory "$stage" --no-same-owner
  extracted="$stage/rowlight-$version-linux-x64-cloud"
  [[ -x "$extracted/parquetviewer" && -f "$extracted/scripts/install-linux.sh" ]] || fail 'The archive is missing its application or installer.'
fi
if $check_only; then printf 'Verification complete. Nothing was installed.\n'; exit 0; fi
if [[ $install_mode == arch ]]; then
  printf 'Installing with pacman. It will ask for your password and confirmation.\n'
  # bash reads this script from a pipe; pacman must read confirmation from the terminal.
  sudo pacman -U "$stage/$asset" </dev/tty
else
  printf 'Installing for your user account…\n'
  bash "$extracted/scripts/install-linux.sh" --skip-build
fi
printf 'Open Rowlight from your application menu.\n'
