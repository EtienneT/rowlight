[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Run this installation test on an isolated GitHub Actions Windows runner.' }
$projectRoot = Split-Path $PSScriptRoot -Parent
$version = ((Get-Content (Join-Path $projectRoot 'Cargo.toml') -Raw) | Select-String '(?m)^version = "([^"]+)"').Matches[0].Groups[1].Value
$setup = Join-Path $projectRoot "dist/tablelamp-$version-windows-x64-cloud-setup.exe"
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('tablelamp-package-' + [guid]::NewGuid().ToString('N'))
$installDirectory = Join-Path $testRoot 'installed'
$extensions = @('.parquet', '.parq', '.ipc', '.arrow', '.feather', '.csv', '.tsv')
$preferences = Join-Path $env:APPDATA 'parquetviewer/settings.json'
$installed = $false

function Get-FileDefaults {
    $defaults = [ordered]@{}
    foreach ($extension in $extensions) {
        $choice = Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\$extension\UserChoice" -ErrorAction SilentlyContinue
        $user = Get-Item "HKCU:\Software\Classes\$extension" -ErrorAction SilentlyContinue
        $machine = Get-Item "HKLM:\Software\Classes\$extension" -ErrorAction SilentlyContinue
        $defaults[$extension] = [ordered]@{
            choice = $choice.ProgId
            hash = $choice.Hash
            user = if ($user) { $user.GetValue('') }
            machine = if ($machine) { $machine.GetValue('') }
        }
    }
    return ConvertTo-Json $defaults -Depth 5 -Compress
}

function Assert-FileDefaults {
    if ((Get-FileDefaults) -ne $script:previousDefaults) { throw 'Existing file defaults changed.' }
}

if (-not (Test-Path $setup)) { throw 'Build the Windows cloud installer first.' }
if (Test-Path 'HKCU:\Software\Classes\ParquetViewer.Parquet') { throw 'Use a clean Windows account; an existing viewer registration must be preserved.' }
if (Test-Path 'HKCU:\Software\ParquetViewer\Capabilities') { throw 'Use a clean Windows account; existing capabilities must be preserved.' }
if (Test-Path $preferences) { throw 'Existing preferences must be preserved; use a clean Windows account.' }
New-Item -ItemType Directory $testRoot | Out-Null
foreach ($extension in @('.parquet', '.csv')) {
    New-Item "HKCU:\Software\Classes\$extension" -Force | Out-Null
    Set-Item "HKCU:\Software\Classes\$extension" -Value 'TablelampSmoke.ExistingViewer'
}
New-Item -ItemType Directory (Split-Path $preferences -Parent) -Force | Out-Null
'{"association_prompted":true,"appearance":"dark"}' | Set-Content $preferences -Encoding utf8NoBOM
$preferencesHash = (Get-FileHash $preferences).Hash
$script:previousDefaults = Get-FileDefaults
try {
    $legacyAsset = 'rowlight-0.1.0-alpha.1-windows-x64-cloud-setup.exe'
    $legacyRelease = 'https://github.com/EtienneT/tablelamp/releases/download/v0.1.0-alpha.1'
    $legacySetup = Join-Path $testRoot $legacyAsset
    $checksumFile = Join-Path $testRoot 'SHA256SUMS'
    Invoke-WebRequest -Uri "$legacyRelease/SHA256SUMS" -OutFile $checksumFile
    $checksums = Get-Content $checksumFile -Raw
    $checksumPattern = '(?m)^([a-fA-F0-9]{64})\s+' + [regex]::Escape($legacyAsset) + '\r?$'
    $matching = [regex]::Matches($checksums, $checksumPattern)
    if ($matching.Count -ne 1) { throw 'Missing legacy installer checksum.' }
    Invoke-WebRequest -Uri "$legacyRelease/$legacyAsset" -OutFile $legacySetup
    if ((Get-FileHash $legacySetup -Algorithm SHA256).Hash -ne $matching[0].Groups[1].Value) { throw 'Legacy installer checksum mismatch.' }
    foreach ($installer in @($legacySetup, $setup)) {
        $process = Start-Process $installer -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/NOICONS', "/DIR=`"$installDirectory`"", '/GROUP=Tablelamp Package Test') -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode)" }
        $installed = $true
        Assert-FileDefaults
        if ((Get-FileHash $preferences).Hash -ne $preferencesHash) { throw 'Saved preferences changed during installation or upgrade.' }
    }
    $binary = Join-Path $installDirectory 'parquetviewer.exe'
    if ((Get-FileHash $binary).Hash -ne (Get-FileHash (Join-Path $projectRoot 'dist/windows/parquetviewer.exe')).Hash) { throw 'Upgrade installed the wrong executable.' }
    if ((Get-Item $binary).VersionInfo.ProductName -ne 'Tablelamp') { throw 'Incorrect executable branding.' }
    if ((Get-Item 'HKCU:\Software\ParquetViewer\Capabilities').GetValue('ApplicationName') -ne 'Tablelamp') { throw 'Incorrect Default Apps branding.' }
    $command = (Get-Item 'HKCU:\Software\Classes\ParquetViewer.Parquet\shell\open\command').GetValue('')
    if ($command -ne "`"$binary`" `"%1`"") { throw 'Incorrect file-open command.' }
    foreach ($extension in $extensions) {
        if ((Get-Item "HKCU:\Software\Classes\$extension\OpenWithProgids").GetValueNames() -notcontains 'ParquetViewer.Parquet') { throw "Missing Open With registration: $extension" }
    }
    $versionOutput = Join-Path $testRoot 'version.txt'
    $process = Start-Process $binary -ArgumentList '--version' -Wait -PassThru -RedirectStandardOutput $versionOutput
    if ($process.ExitCode -ne 0 -or (Get-Content $versionOutput -Raw).Trim() -ne "Tablelamp $version (cloud)") { throw 'Installed application startup failed.' }
    if (-not (Test-Path (Join-Path $installDirectory 'THIRD_PARTY_NOTICES.html'))) { throw 'Installed license notices missing.' }
    $process = Start-Process (Join-Path $installDirectory 'unins000.exe') -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw 'Uninstaller failed.' }
    $installed = $false
    if (Test-Path $binary) { throw 'Installed executable remains after removal.' }
    if (Test-Path 'HKCU:\Software\Classes\ParquetViewer.Parquet') { throw 'Document registration remains after removal.' }
    if (Test-Path 'HKCU:\Software\ParquetViewer\Capabilities') { throw 'Capabilities remain after removal.' }
    foreach ($extension in $extensions) {
        $entry = Get-Item "HKCU:\Software\Classes\$extension\OpenWithProgids" -ErrorAction SilentlyContinue
        if ($entry -and $entry.GetValueNames() -contains 'ParquetViewer.Parquet') { throw "Open With registration remains: $extension" }
    }
    Assert-FileDefaults
    if ((Get-FileHash $preferences).Hash -ne $preferencesHash) { throw 'Saved preferences changed.' }
    Write-Output "Passed Tablelamp $version installation, legacy upgrade, startup, default preservation and removal."
} finally {
    if ($installed -and (Test-Path (Join-Path $installDirectory 'unins000.exe'))) {
        $process = Start-Process (Join-Path $installDirectory 'unins000.exe') -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw 'Failed to remove the isolated test installation.' }
    }
    Remove-Item $testRoot -Recurse -Force
}
