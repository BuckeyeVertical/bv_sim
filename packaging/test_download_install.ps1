# Exercise the download installer with a local ZIP; never contacts GitHub or changes PATH.
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Binary)

$ErrorActionPreference = 'Stop'
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$versionOutput = & $Binary --version
if ($LASTEXITCODE -ne 0 -or $versionOutput -notmatch '^bv-sim ([0-9]+\.[0-9]+\.[0-9]+)$') {
    throw 'Invalid executable version.'
}
$releaseVersion = $Matches[1]
$root = Join-Path ([IO.Path]::GetTempPath()) ('bv-sim-bootstrap-test-' + [Guid]::NewGuid().ToString('N'))
$bundle = Join-Path $root "a user's downloads [test]/bv-sim-$releaseVersion-x86_64-pc-windows-msvc"
$installed = Join-Path $root "a user's programs [test]/BVSim"
$script:fixtureArchive = Join-Path $root 'fixture.zip'
$script:downloadedArchive = $null
$script:failDownload = $false
$script:omitAsset = $false
$originalArchitecture = $env:PROCESSOR_ARCHITECTURE
$originalWow64Architecture = $env:PROCESSOR_ARCHITEW6432

function Invoke-RestMethod {
    param([string]$Uri, [hashtable]$Headers)
    if ($Uri -ne 'https://api.github.com/repos/BuckeyeVertical/bv_sim/releases/latest') {
        throw "Unexpected release request: $Uri"
    }
    $assets = @()
    if (-not $script:omitAsset) {
        $assets = @([pscustomobject]@{
            name = "bv-sim-$releaseVersion-x86_64-pc-windows-msvc.zip"
            browser_download_url = 'https://example.invalid/release.zip'
        })
    }
    return [pscustomobject]@{ tag_name = "v$releaseVersion"; assets = $assets }
}

function Invoke-WebRequest {
    param([switch]$UseBasicParsing, [string]$Uri, [string]$OutFile)
    if ($Uri -ne 'https://example.invalid/release.zip') { throw "Unexpected asset request: $Uri" }
    $script:downloadedArchive = $OutFile
    if ($script:failDownload) { throw 'Simulated download failure' }
    Copy-Item -LiteralPath $script:fixtureArchive -Destination $OutFile
}

try {
    foreach ($directory in @('bin', 'assets', 'scenarios')) {
        New-Item -ItemType Directory -Path (Join-Path $bundle $directory) -Force | Out-Null
    }
    Copy-Item -LiteralPath $Binary -Destination (Join-Path $bundle 'bin/bv-sim.exe')
    Set-Content -LiteralPath (Join-Path $bundle 'assets/asset.txt') -Value 'asset'
    Set-Content -LiteralPath (Join-Path $bundle 'scenarios/suas_2026.toml') -Value 'scenario'
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'install.ps1') -Destination $bundle
    Compress-Archive -LiteralPath $bundle -DestinationPath $script:fixtureArchive -CompressionLevel Fastest
    $bootstrap = Join-Path $PSScriptRoot '../install.ps1'
    $bootstrapContents = Get-Content -LiteralPath $bootstrap -Raw
    # Exercise both file execution and the README's piped Invoke-Expression path.
    foreach ($mode in @('file', 'expression')) {
        if ($mode -eq 'file') {
            & $bootstrap -NoPath -InstallDir $installed
        } else {
            "& {`n$bootstrapContents`n} -NoPath -InstallDir `$installed" | Invoke-Expression
        }
    }
    foreach ($file in @('bin/bv-sim.exe', 'assets/asset.txt', 'scenarios/suas_2026.toml')) {
        if (-not (Test-Path -LiteralPath (Join-Path $installed $file))) { throw "Installed $file missing." }
    }
    if (Test-Path -LiteralPath (Split-Path -Parent $script:downloadedArchive)) { throw 'Download directory was retained.' }
    $script:failDownload = $true
    $failed = $false
    try { & $bootstrap -NoPath -InstallDir $installed } catch {
        if (-not $_.ToString().Contains('Simulated download failure')) { throw }
        $failed = $true
    }
    if (-not $failed) { throw 'A failed download was not reported.' }
    if (Test-Path -LiteralPath (Split-Path -Parent $script:downloadedArchive)) { throw 'Failed download was not cleaned up.' }
    $script:omitAsset = $true
    $failed = $false
    try { & $bootstrap -NoPath -InstallDir $installed } catch {
        if (-not $_.ToString().Contains('does not include')) { throw }
        $failed = $true
    }
    if (-not $failed) { throw 'A missing release asset was not reported.' }

    # A missing asset confirms supported architectures got past the platform
    # check, without downloading or installing another copy of the bundle.
    foreach ($case in @(
        @{ Process = 'AMD64'; Native = $null; Supported = $true },
        @{ Process = 'x86'; Native = 'AMD64'; Supported = $true },
        @{ Process = 'ARM64'; Native = $null; Supported = $false },
        @{ Process = 'AMD64'; Native = 'ARM64'; Supported = $false },
        @{ Process = 'x86'; Native = 'ARM64'; Supported = $false },
        @{ Process = 'x86'; Native = $null; Supported = $false },
        @{ Process = $null; Native = $null; Supported = $false }
    )) {
        $env:PROCESSOR_ARCHITECTURE = $case.Process
        $env:PROCESSOR_ARCHITEW6432 = $case.Native
        $expectedError = if ($case.Supported) { 'does not include' } else { 'requires x86_64 Windows' }
        $failed = $false
        try {
            "& {`n$bootstrapContents`n} -NoPath -InstallDir `$installed" | Invoke-Expression
        } catch {
            if (-not $_.ToString().Contains($expectedError)) { throw }
            $failed = $true
        }
        if (-not $failed) { throw "Architecture check did not report: $expectedError" }
    }
    Write-Host 'Windows download installer: complete bundle, Invoke-Expression, architecture, missing asset, and failed download checks passed.'
} finally {
    $env:PROCESSOR_ARCHITECTURE = $originalArchitecture
    $env:PROCESSOR_ARCHITEW6432 = $originalWow64Architecture
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
