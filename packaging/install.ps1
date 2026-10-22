# User-local install from an extracted Windows release or a built source checkout.
[CmdletBinding()]
param(
    [switch]$NoPath,
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\BVSim')
)

$ErrorActionPreference = 'Stop'
$bundle = if (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'VERSION')) {
    $PSScriptRoot
} else {
    Split-Path -Parent $PSScriptRoot
}
$version = (Get-Content -LiteralPath (Join-Path $bundle 'VERSION') -Raw).Trim()
if ($version -notmatch '^[0-9A-Za-z._-]+$') { throw 'Invalid release VERSION' }
$binary = Join-Path $bundle 'bin/bv-sim.exe'
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    $binary = Join-Path $bundle 'target/release/bv-sim.exe'
}
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    throw 'Extract the complete Windows release, or first run cargo build --release -p bv-sim.'
}
foreach ($directory in @('assets', 'scenarios')) {
    if (-not (Test-Path -LiteralPath (Join-Path $bundle $directory) -PathType Container)) {
        throw "Missing $directory. Extract the complete release archive."
    }
}
$InstallDir = [IO.Path]::GetFullPath($InstallDir)
if ($InstallDir.TrimEnd('\', '/') -eq $bundle.TrimEnd('\', '/')) {
    throw 'Choose an installation directory different from the extracted download.'
}
$bin = Join-Path $InstallDir 'bin'
New-Item -ItemType Directory -Path $bin -Force | Out-Null
# A running Windows executable is locked: fail before replacing its asset data.
Copy-Item -LiteralPath $binary -Destination (Join-Path $bin 'bv-sim.exe') -Force
# Assets are immutable release data; replace the directories to avoid stale files
# from older versions. User configuration belongs outside the installed bundle.
foreach ($directory in @('assets', 'scenarios')) {
    $destination = Join-Path $InstallDir $directory
    if (Test-Path -LiteralPath $destination) {
        Remove-Item -LiteralPath $destination -Recurse -Force
    }
    Copy-Item -LiteralPath (Join-Path $bundle $directory) -Destination $InstallDir -Recurse
}
Copy-Item -LiteralPath (Join-Path $bundle 'VERSION') -Destination $InstallDir -Force

if (-not $NoPath) {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw 'Updating the user PATH requires Windows. Use -NoPath for a file-only installation.'
    }
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @($userPath -split ';' | Where-Object { $_.Trim() })
    if (-not ($entries | Where-Object { $_.Trim().TrimEnd('\') -ieq $bin.TrimEnd('\') })) {
        [Environment]::SetEnvironmentVariable('Path', (($entries + $bin) -join ';'), 'User')
    }
    if (-not (($env:Path -split ';') | Where-Object { $_.Trim().TrimEnd('\') -ieq $bin.TrimEnd('\') })) {
        $env:Path = "$bin;$env:Path"
    }
}
Write-Host "Installed bv-sim $version to $InstallDir"
if ($NoPath) {
    Write-Host "Add $bin to your PATH, then run: bv-sim doctor"
} else {
    Write-Host 'Open a new terminal, then run: bv-sim doctor'
}
