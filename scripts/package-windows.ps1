# Build natively with Visual Studio C++/Windows SDK and Inno Setup 6.
[CmdletBinding()]
param([switch]$SkipBuild, [switch]$Cloud)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Push-Location $projectRoot
try {
    $suffix = if ($Cloud) { '-cloud' } else { '' }
    $featureArguments = if ($Cloud) { @('--features', 'cloud') } else { @() }
    if (-not $SkipBuild) {
        # Keep the portable executable independent of the Visual C++ runtime.
        $previousFlags = $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS
        try {
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = "$previousFlags -C target-feature=+crt-static".Trim()
            cargo build --locked --release --target x86_64-pc-windows-msvc --bin parquetviewer @featureArguments
            if ($LASTEXITCODE -ne 0) { throw 'Windows release build failed.' }
        } finally {
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = $previousFlags
        }
    }
    $binary = Join-Path $projectRoot 'target/x86_64-pc-windows-msvc/release/parquetviewer.exe'
    if (-not (Test-Path $binary)) { throw "Missing release executable: $binary" }
    $stage = Join-Path $projectRoot 'dist/windows'
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item $binary (Join-Path $stage 'parquetviewer.exe') -Force
    Copy-Item 'LICENSE' $stage -Force
    Copy-Item 'packaging/windows/README.txt' $stage -Force
    Compress-Archive -Path "$stage/*" -DestinationPath "dist/parquetviewer-windows-x64$suffix.zip" -Force
    $compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($compiler) { $iscc = $compiler.Source }
    else { $iscc = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6/ISCC.exe' }
    if (-not (Test-Path $iscc)) { throw 'Install Inno Setup 6 to build the setup executable.' }
    $metadata = cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read package version.' }
    $version = ($metadata.packages | Where-Object name -eq 'parquetviewer').version
    & $iscc "/DAppVersion=$version" "/DPackageSuffix=$suffix" 'packaging/windows/parquetviewer.iss'
    if ($LASTEXITCODE -ne 0) { throw 'Windows installer compilation failed.' }
    Write-Output "Created dist/parquetviewer-windows-x64$suffix.zip and dist/parquetviewer-windows-x64$suffix-setup.exe"
} finally { Pop-Location }
