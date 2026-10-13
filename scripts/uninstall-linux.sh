#!/usr/bin/env bash
# Remove the per-user archive installation, retaining application preferences.
set -euo pipefail
app_data_dir=${XDG_DATA_HOME:-"$HOME/.local/share"}
app_id=com.parquetviewer.ParquetViewer
previous_default=""
if [[ -f "$app_data_dir/parquetviewer/previous-default" ]]; then
  IFS= read -r previous_default < "$app_data_dir/parquetviewer/previous-default" || true
fi
if command -v xdg-mime >/dev/null && [[ -n "$previous_default" && "$previous_default" != "$app_id.desktop" ]]; then
  for mime_type in application/vnd.apache.parquet application/x-parquet; do
    if [[ $(xdg-mime query default "$mime_type") == "$app_id.desktop" ]]; then
      xdg-mime default "$previous_default" "$mime_type"
    fi
  done
fi
rm -f -- "$HOME/.local/bin/parquetviewer" \
  "$app_data_dir/applications/$app_id.desktop" \
  "$app_data_dir/mime/packages/parquetviewer.xml" \
  "$app_data_dir/icons/hicolor/scalable/apps/$app_id.svg" \
  "$app_data_dir/parquetviewer/LICENSE" \
  "$app_data_dir/parquetviewer/THIRD_PARTY_NOTICES.html" \
  "$app_data_dir/parquetviewer/previous-default"
if [[ -d "$app_data_dir/mime" ]] && command -v update-mime-database >/dev/null; then update-mime-database "$app_data_dir/mime"; fi
if [[ -d "$app_data_dir/applications" ]] && command -v update-desktop-database >/dev/null; then update-desktop-database "$app_data_dir/applications"; fi
rmdir -- "$app_data_dir/parquetviewer" 2>/dev/null || true
printf 'Removed the per-user Rowlight installation. Preferences were kept.\n'
