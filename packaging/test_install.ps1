[CmdletBinding()]
param([string]$Binary)

$ErrorActionPreference = 'Stop'
$releaseVersion = (Get-Content -LiteralPath (Join-Path $PSScriptRoot '../VERSION') -Raw).Trim()
$windows = [Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT
if ($windows -and -not $Binary) { throw 'Pass -Binary with a built bv-sim.exe on Windows.' }
if ($Binary) { $Binary = (Resolve-Path -LiteralPath $Binary).Path }
$root = Join-Path ([IO.Path]::GetTempPath()) ('bv-sim-install-' + [Guid]::NewGuid().ToString('N'))
$bundle = Join-Path $root "a user's downloads [test]/release"
$installed = Join-Path $root "a user's programs [test]/BVSim"
$originalUserPath = if ($windows) { [Environment]::GetEnvironmentVariable('Path', 'User') } else { $null }
$originalProcessPath = $env:Path
try {
    New-Item -ItemType Directory -Path (Join-Path $bundle 'bin') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $bundle 'assets') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $bundle 'scenarios') -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $bundle 'VERSION') -Value $releaseVersion
    Set-Content -LiteralPath (Join-Path $bundle 'assets/asset.txt') -Value 'asset'
    Set-Content -LiteralPath (Join-Path $bundle 'scenarios/suas_2026.toml') -Value 'scenario'
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'install.ps1') -Destination $bundle
    if ($Binary) {
        Copy-Item -LiteralPath $Binary -Destination (Join-Path $bundle 'bin/bv-sim.exe')
    } else {
        Set-Content -LiteralPath (Join-Path $bundle 'bin/bv-sim.exe') -Value 'file-copy fixture'
    }
    & (Join-Path $bundle 'install.ps1') -InstallDir $installed -NoPath
    Set-Content -LiteralPath (Join-Path $installed 'assets/stale.txt') -Value 'obsolete asset'
    & (Join-Path $bundle 'install.ps1') -InstallDir $installed -NoPath
    if (Test-Path -LiteralPath (Join-Path $installed 'assets/stale.txt')) { throw 'Stale release data was retained.' }
    if ($windows) {
        & (Join-Path $bundle 'install.ps1') -InstallDir $installed
        & (Join-Path $bundle 'install.ps1') -InstallDir $installed
        $entries = [Environment]::GetEnvironmentVariable('Path', 'User') -split ';'
        if (@($entries | Where-Object { $_ -ieq (Join-Path $installed 'bin') }).Count -ne 1) {
            throw 'User PATH must contain exactly one installation entry.'
        }
    }
    Remove-Item -LiteralPath $bundle -Recurse -Force
    if (-not (Test-Path -LiteralPath (Join-Path $installed 'assets/asset.txt'))) { throw 'Installed assets missing.' }
    if (-not (Test-Path -LiteralPath (Join-Path $installed 'scenarios/suas_2026.toml'))) { throw 'Installed scenario missing.' }
    if ($windows) {
        Push-Location ([IO.Path]::GetTempPath())
        try {
            $version = & (Join-Path $installed 'bin/bv-sim.exe') --version
            if ($LASTEXITCODE -ne 0 -or $version -ne "bv-sim $releaseVersion") { throw 'Installed executable did not start.' }
            # Docker is deliberately absent: doctor still prints the resolved data
            # paths before returning the expected missing-Docker error.
            $PSNativeCommandUseErrorActionPreference = $false
            $report = (& (Join-Path $installed 'bin/bv-sim.exe') --docker (Join-Path $root 'missing-docker.exe') doctor 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 1) { throw 'Expected the missing-Docker diagnostic.' }
            foreach ($directory in @('assets', 'scenarios')) {
                if (-not $report.Contains((Join-Path $installed $directory))) { throw "Incorrect installed $directory path: $report" }
            }
        } finally { Pop-Location }
    }
    Write-Host 'Windows installer: relocation, quoting, repeat installation, and stale-data cleanup passed.'
} finally {
    if ($windows) { [Environment]::SetEnvironmentVariable('Path', $originalUserPath, 'User') }
    $env:Path = $originalProcessPath
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
