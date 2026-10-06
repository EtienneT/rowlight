Parquet Viewer

Run parquetviewer.exe, then use the top-left menu > Open file, Ctrl+O, or drop
a Parquet, IPC/Feather v2, CSV or TSV file onto its window.
The portable ZIP does not change file associations or add Start menu entries.
Use the setup executable for a per-user installation with file type registration.

On first launch, the viewer asks whether to associate Parquet files. Choose in
Windows Settings opens Default Apps, where you select Parquet Viewer for .parquet
and .parq. No thanks remembers your choice. Reopen the offer from the app menu.
Alternatively, right-click a Parquet file > Open with > Choose another app >
Parquet Viewer > Always. Windows owns the final default-app choice.

Preferences: %APPDATA%\parquetviewer\settings.json
Windows 10 or later, x64, with a working DirectX 11 graphics driver.

The cloud download also opens S3, Azure, GCS and HTTP(S) Parquet through the
Open remote Parquet menu item. Use existing provider credentials or signed URLs.
No secrets are saved; signed URLs are excluded from recent files.
