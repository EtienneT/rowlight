; Build on Windows with Inno Setup 6. Per-user installation requires no elevation.
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef PackageSuffix
  #define PackageSuffix ""
#endif
[Setup]
AppId=com.parquetviewer.ParquetViewer
AppName=Tablelamp
AppVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Tablelamp
UsePreviousAppDir=yes
DefaultGroupName=Tablelamp
PrivilegesRequired=lowest
MinVersion=10.0
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\..\dist
OutputBaseFilename=tablelamp-{#AppVersion}-windows-x64{#PackageSuffix}-setup
SetupIconFile=icon.ico
UninstallDisplayIcon={app}\parquetviewer.exe
Compression=lzma2
SolidCompression=yes
ChangesAssociations=yes
LicenseFile=..\..\LICENSE

[Files]
Source: "..\..\dist\windows\parquetviewer.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\THIRD_PARTY_NOTICES.html"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Tablelamp"; Filename: "{app}\parquetviewer.exe"

[Registry]
Root: HKCU; Subkey: "Software\Classes\ParquetViewer.Parquet"; ValueType: string; ValueName: ""; ValueData: "Tabular data file"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\ParquetViewer.Parquet\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\parquetviewer.exe"",0"
Root: HKCU; Subkey: "Software\Classes\ParquetViewer.Parquet\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\parquetviewer.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\.parquet\OpenWithProgids"; ValueType: string; ValueName: "ParquetViewer.Parquet"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.parq\OpenWithProgids"; ValueType: string; ValueName: "ParquetViewer.Parquet"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\ParquetViewer\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "Tablelamp"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\ParquetViewer\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "View Parquet, IPC, CSV and TSV files"
Root: HKCU; Subkey: "Software\ParquetViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".parquet"; ValueData: "ParquetViewer.Parquet"
Root: HKCU; Subkey: "Software\ParquetViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".parq"; ValueData: "ParquetViewer.Parquet"
Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "Parquet Viewer"; ValueData: "Software\ParquetViewer\Capabilities"; Flags: uninsdeletevalue
; Preserve legacy registration identity and AppId for upgrades.

Root: HKCU; Subkey: "Software\Classes\.ipc\OpenWithProgids"; ValueType: string; ValueName: "ParquetViewer.Parquet"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\ParquetViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".ipc"; ValueData: "ParquetViewer.Parquet"
Root: HKCU; Subkey: "Software\Classes\.arrow\OpenWithProgids"; ValueType: string; ValueName: "ParquetViewer.Parquet"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\ParquetViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".arrow"; ValueData: "ParquetViewer.Parquet"
Root: HKCU; Subkey: "Software\Classes\.feather\OpenWithProgids"; ValueType: string; ValueName: "ParquetViewer.Parquet"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\ParquetViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".feather"; ValueData: "ParquetViewer.Parquet"
Root: HKCU; Subkey: "Software\Classes\.csv\OpenWithProgids"; ValueType: string; ValueName: "ParquetViewer.Parquet"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\ParquetViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".csv"; ValueData: "ParquetViewer.Parquet"
Root: HKCU; Subkey: "Software\Classes\.tsv\OpenWithProgids"; ValueType: string; ValueName: "ParquetViewer.Parquet"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\ParquetViewer\Capabilities\FileAssociations"; ValueType: string; ValueName: ".tsv"; ValueData: "ParquetViewer.Parquet"

[Run]
Filename: "{app}\parquetviewer.exe"; Description: "Launch Tablelamp"; Flags: nowait postinstall skipifsilent
