#!/usr/bin/env bash
# Register Open With support. The application asks before changing defaults.
set -euo pipefail
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
app_data_dir=${XDG_DATA_HOME:-"$HOME/.local/share"}
app_config_dir=${XDG_CONFIG_HOME:-"$HOME/.config"}
app_bin_dir="$HOME/.local/bin"
app_id=com.parquetviewer.ParquetViewer
skip_build=false
cloud=false
for argument in "$@"; do
  case "$argument" in --skip-build) skip_build=true ;; --cloud) cloud=true ;; *) printf 'Unknown option: %s\n' "$argument" >&2; exit 1 ;; esac
done
features=()
if $cloud; then features=(--features cloud); fi
for command in xdg-mime update-mime-database update-desktop-database; do
  command -v "$command" >/dev/null || { printf 'Required command missing: %s\n' "$command" >&2; exit 1; }
done
app_source="$project_root/target/release/parquetviewer"
if [[ -x "$project_root/parquetviewer" ]]; then
  app_source="$project_root/parquetviewer"
elif ! $skip_build; then
  (cd "$project_root" && cargo build --release --locked "${features[@]}")
fi
[[ -x "$app_source" ]] || { printf 'Build the release binary first.\n' >&2; exit 1; }
# Registering a new handler can change a desktop's implicit fallback. Preserve
# existing choices even when they had not been written explicitly to mimeapps.
previous_default=$(xdg-mime query default application/vnd.apache.parquet)
mime_types=(application/vnd.apache.parquet application/x-parquet application/vnd.apache.arrow.file application/x-feather text/csv text/tab-separated-values)
previous_mime_defaults=()
for mime_type in "${mime_types[@]}"; do previous_mime_defaults+=("$(xdg-mime query default "$mime_type")"); done
mkdir -p "$app_bin_dir" "$app_config_dir" "$app_data_dir/applications" "$app_data_dir/mime/packages" "$app_data_dir/icons/hicolor/scalable/apps" "$app_data_dir/parquetviewer"
if [[ ! -f "$app_data_dir/parquetviewer/previous-default" ]]; then
  printf '%s\n' "$previous_default" > "$app_data_dir/parquetviewer/previous-default"
fi
install -m755 "$app_source" "$app_bin_dir/parquetviewer"
# Use an absolute, escaped executable path; the file manager need not inherit PATH.
desktop_exec="$app_bin_dir/parquetviewer"
desktop_exec=${desktop_exec//\\/\\\\}
desktop_exec=${desktop_exec//\"/\\\"}
desktop_exec=${desktop_exec//\$/\\\$}
desktop_exec=${desktop_exec//\`/\\\`}
desktop_exec=${desktop_exec//%/%%}
while IFS= read -r line; do
  if [[ "$line" == Exec=* ]]; then printf 'Exec="%s" %%f\n' "$desktop_exec"; else printf '%s\n' "$line"; fi
done < "$project_root/packaging/linux/$app_id.desktop" > "$app_data_dir/applications/$app_id.desktop"
install -m644 "$project_root/packaging/linux/parquet.xml" "$app_data_dir/mime/packages/parquetviewer.xml"
install -m644 "$project_root/packaging/linux/$app_id.svg" "$app_data_dir/icons/hicolor/scalable/apps/$app_id.svg"
update-mime-database "$app_data_dir/mime"
update-desktop-database "$app_data_dir/applications"
for index in "${!mime_types[@]}"; do
  previous="${previous_mime_defaults[$index]}"
  if [[ -n "$previous" && "$previous" != "$app_id.desktop" ]]; then
    xdg-mime default "$previous" "${mime_types[$index]}"
  fi
done
printf 'Installed %s. On first launch, choose whether to make it the default for Parquet files.\n' "$app_bin_dir/parquetviewer"
