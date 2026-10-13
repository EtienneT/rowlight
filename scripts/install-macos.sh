#!/usr/bin/env bash
# Install and register with Finder for the current user, without sudo.
set -euo pipefail
[[ $(uname -s) == Darwin ]] || { printf 'Run this installer on a Mac.\n' >&2; exit 1; }
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
"$project_root/scripts/package-macos.sh" "$@"
destination="$HOME/Applications/Parquet Viewer.app"
mkdir -p "$HOME/Applications"
ditto "$project_root/dist/Parquet Viewer.app" "$destination"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$destination"
printf 'Installed %s. Finder now offers Tablelamp for .parquet and .parq files.\n' "$destination"
