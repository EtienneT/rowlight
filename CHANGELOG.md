# Changelog

## 0.1.0-alpha.2 — Unreleased

- Rename the product to Tablelamp with a lamp-and-grid SVG icon; preserve application, executable and preference identities for upgrades.

- Add verified quick-install commands, a small synthetic demo dataset and social preview.
- Normalize Windows build paths and keep public validation records free of private test-session details.
- Retain the existing read-only viewer behavior and Linux/Windows platform scope.

## 0.1.0-alpha.1

First public alpha for Linux x64 and Windows x64, published under the Rowlight name.

- Read-only Parquet, Arrow IPC / Feather v2, CSV and TSV browsing.
- Paged reads, original column order, typed columns and guarded All mode.
- Column distributions, expanded charts, cell inspection and file overview.
- Column search, visibility, resizing, filters and keyboard navigation.
- Read-only SQL mode over the opened source and local Hive Parquet folders.
- Remote Parquet from S3, Azure, GCS and HTTP(S) in the cloud download.
- Saved preferences and views; Omarchy, system, light and dark appearance.
- Compact error messages with expandable details and editable SQL recovery.

macOS remains experimental source tooling. Iceberg and remote Hive folders are
not supported. Page sorting and plot/sample scope are shown in the app.
