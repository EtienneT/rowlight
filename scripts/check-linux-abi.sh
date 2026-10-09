#!/usr/bin/env bash
# Reject archives linked against a newer glibc than the release baseline.
set -euo pipefail
binary=${1:?Usage: check-linux-abi.sh EXECUTABLE}
baseline=2.39
versions=$(LC_ALL=C readelf --version-info "$binary")
required=$(printf '%s\n' "$versions" | sed -n 's/.*Name: GLIBC_\([0-9.]*\)  Flags: none.*/\1/p' | sort -Vu | tail -1)
[[ -n "$required" ]] || { printf 'Cannot determine glibc requirements.\n' >&2; exit 1; }
highest=$(printf '%s\n' "$baseline" "$required" | sort -V | tail -1)
[[ "$highest" == "$baseline" ]] || {
    printf 'Requires glibc %s; release baseline is %s. Use scripts/package-linux-container.sh.\n' "$required" "$baseline" >&2
    exit 1
}
printf 'Linux ABI check passed: glibc %s <= %s\n' "$required" "$baseline"
