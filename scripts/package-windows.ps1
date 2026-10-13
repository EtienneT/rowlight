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
            # Panic locations must not expose the release builder's local paths.
            # Map both separators because rustc remapping is textual on Windows.
            $remaps = @()
            foreach ($entry in @(@($projectRoot, '/tablelamp'), @($env:CARGO_HOME, '/cargo'), @($env:RUSTUP_HOME, '/rustup'))) {
                if ($entry[0]) {
                    foreach ($prefix in @($entry[0], $entry[0].Replace('\', '/')) | Select-Object -Unique) {
                        $remaps += '--remap-path-prefix="' + $prefix + '=' + $entry[1] + '"'
                    }
                }
            }
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = "$previousFlags -C target-feature=+crt-static $($remaps -join ' ')".Trim()
            cargo build --locked --release --target x86_64-pc-windows-msvc --bin parquetviewer @featureArguments
            if ($LASTEXITCODE -ne 0) { throw 'Windows release build failed.' }
        } finally {
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = $previousFlags
        }
    }
    $targetDirectory = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $projectRoot 'target' }
    $binary = Join-Path $targetDirectory 'x86_64-pc-windows-msvc/release/parquetviewer.exe'
    if (-not (Test-Path $binary)) { throw "Missing release executable: $binary" }
    $metadata = cargo metadata --locked --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read package version.' }
    $version = ($metadata.packages | Where-Object name -eq 'parquetviewer').version
    $variant = if ($Cloud) { 'cloud' } else { 'core' }
    # PowerShell does not reliably wait for GUI-subsystem executables in a pipe.
    # Wait explicitly and capture output for the GUI-subsystem executable.
    $versionOutput = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
    $versionError = "$versionOutput.err"
    try {
        $process = Start-Process $binary -ArgumentList '--version' -Wait -PassThru -RedirectStandardOutput $versionOutput -RedirectStandardError $versionError
        if ($process.ExitCode -ne 0 -or (Get-Content $versionOutput -Raw).Trim() -ne "Tablelamp $version ($variant)") { throw 'Release executable version or feature set does not match.' }
    } finally {
        Remove-Item $versionOutput, $versionError -Force -ErrorAction SilentlyContinue
    }
    $licenseHash = (Get-Content 'packaging/licenses.lock.sha256' -Raw).Trim()
    if ((Get-FileHash Cargo.lock -Algorithm SHA256).Hash.ToLowerInvariant() -ne $licenseHash) { throw 'Regenerate third-party notices after changing Cargo.lock.' }
    $name = "tablelamp-$version-windows-x64$suffix"
    $stage = Join-Path $projectRoot 'dist/windows'
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item $binary (Join-Path $stage 'parquetviewer.exe') -Force
    Copy-Item 'LICENSE' $stage -Force
    Copy-Item 'THIRD_PARTY_NOTICES.html' $stage -Force
    Copy-Item 'packaging/windows/README.txt' $stage -Force
    Compress-Archive -Path "$stage/*" -DestinationPath "dist/$name.zip" -Force
    $compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($compiler) { $iscc = $compiler.Source }
    else { $iscc = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6/ISCC.exe' }
    if (-not (Test-Path $iscc)) { throw 'Install Inno Setup 6 to build the setup executable.' }
    & $iscc "/DAppVersion=$version" "/DPackageSuffix=$suffix" 'packaging/windows/parquetviewer.iss'
    if ($LASTEXITCODE -ne 0) { throw 'Windows installer compilation failed.' }
    foreach ($file in @("$name.zip", "$name-setup.exe")) {
        $hash = (Get-FileHash "dist/$file" -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $file" | Set-Content "dist/$file.sha256" -Encoding utf8NoBOM
    }
    Write-Output "Created dist/$name.zip and dist/$name-setup.exe"
} finally { Pop-Location }
