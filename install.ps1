# Download and install the latest Windows release without a source checkout.
[CmdletBinding()]
param(
    [switch]$NoPath,
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\BVSim')
)

$ErrorActionPreference = 'Stop'
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'The Windows release requires x86_64 Windows.'
}
# RuntimeInformation.OSArchitecture can be missing in Windows PowerShell 5.1.
# Under WOW64, PROCESSOR_ARCHITEW6432 identifies the native OS architecture.
$architecture = if ($env:PROCESSOR_ARCHITEW6432) {
    $env:PROCESSOR_ARCHITEW6432
} else {
    $env:PROCESSOR_ARCHITECTURE
}
if ($architecture -ne 'AMD64') {
    throw 'The Windows release requires x86_64 Windows.'
}
$repository = 'BuckeyeVertical/bv_sim'
Write-Host 'Finding the latest BV Sim release...'
$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repository/releases/latest" -Headers @{
    Accept = 'application/vnd.github+json'
    'User-Agent' = 'bv-sim-installer'
}
if ($release.tag_name -notmatch '^v([0-9]+\.[0-9]+\.[0-9]+)$') {
    throw 'The latest release does not have a valid version tag.'
}
$version = $Matches[1]
$target = 'x86_64-pc-windows-msvc'
$assetName = "bv-sim-$version-$target.zip"
$assets = @($release.assets | Where-Object { $_.name -eq $assetName })
if ($assets.Count -ne 1) { throw "The latest release does not include $assetName." }
$downloadDir = Join-Path ([IO.Path]::GetTempPath()) ('bv-sim-download-' + [Guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path $downloadDir | Out-Null
    $archive = Join-Path $downloadDir 'release.zip'
    Write-Host "Downloading BV Sim $version for $target..."
    Invoke-WebRequest -UseBasicParsing -Uri $assets[0].browser_download_url -OutFile $archive
    Expand-Archive -LiteralPath $archive -DestinationPath $downloadDir
    $installer = Join-Path $downloadDir "bv-sim-$version-$target/install.ps1"
    if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) { throw 'Release installer is missing.' }
    $installerArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $installer, '-InstallDir', $InstallDir)
    if ($NoPath) { $installerArgs += '-NoPath' }
    & powershell.exe @installerArgs
    if ($LASTEXITCODE -ne 0) { throw "BV Sim installation failed (exit $LASTEXITCODE)." }
} finally {
    if (Test-Path -LiteralPath $downloadDir) { Remove-Item -LiteralPath $downloadDir -Recurse -Force }
}
