# Requires PowerShell 5.1 or later. Only the published installer is executed.
param([switch]$Check)

& {
    param([switch]$CheckOnly)
    $ErrorActionPreference = 'Stop'
    $version = '0.1.0-alpha.1'
    $asset = "rowlight-$version-windows-x64-cloud-setup.exe"
    $releaseUrl = "https://github.com/EtienneT/rowlight/releases/download/v$version"
    if (-not $CheckOnly) {
        if ($env:OS -ne 'Windows_NT') { throw 'Official Windows downloads require Windows x64.' }
        $processor = Get-CimInstance Win32_Processor | Select-Object -First 1
        if ($processor.Architecture -ne 9) { throw 'Official Windows downloads require an x64 processor.' }
    }
    $stage = Join-Path ([IO.Path]::GetTempPath()) ('rowlight-install-' + [guid]::NewGuid().ToString('N'))
    $previousTls = [Net.ServicePointManager]::SecurityProtocol
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        [IO.Directory]::CreateDirectory($stage) | Out-Null
        Write-Host "Downloading Rowlight $version..."
        $checksums = Join-Path $stage 'SHA256SUMS'
        $installer = Join-Path $stage $asset
        Invoke-WebRequest -UseBasicParsing -Uri "$releaseUrl/SHA256SUMS" -OutFile $checksums
        Invoke-WebRequest -UseBasicParsing -Uri "$releaseUrl/$asset" -OutFile $installer
        $pattern = '^(?<hash>[a-fA-F0-9]{64})\s+\*?' + [regex]::Escape($asset) + '$'
        $matching = @(Get-Content -LiteralPath $checksums | Where-Object { $_ -match $pattern })
        if ($matching.Count -ne 1) { throw 'The release does not contain one valid checksum for this download.' }
        $expected = [regex]::Match($matching[0], $pattern).Groups['hash'].Value
        $actual = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
        if ($actual -ne $expected) { throw 'Checksum verification failed. Nothing was installed.' }
        Write-Host 'Download checksum verified.'
        if ($CheckOnly) {
            Write-Host 'Verification complete. Nothing was installed.'
            return
        }
        Write-Host 'Opening the installer. Follow its steps, then open Rowlight from Start.'
        $process = Start-Process -FilePath $installer -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw "The installer ended with code $($process.ExitCode)." }
    }
    finally {
        [Net.ServicePointManager]::SecurityProtocol = $previousTls
        if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
    }
} -CheckOnly:$Check
