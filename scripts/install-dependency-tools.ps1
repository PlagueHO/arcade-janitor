# Install the exact checker used by local commands and CI.
#Requires -Version 7.0
[CmdletBinding()]
param([switch]$Check)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$version = '0.22.0'
$root = Split-Path -Parent $PSScriptRoot
$tools = Join-Path $root '.dependency-tools'
$suffix = if ($IsWindows) { '.exe' } else { '' }
$binary = Join-Path $tools "bin/cargo-deny$suffix"

if (Test-Path -LiteralPath $binary) {
    $actual = & $binary --version
    if ($LASTEXITCODE -eq 0 -and $actual -eq "cargo-deny $version") {
        Write-Host "cargo-deny $version is installed."
        return
    }
}
if ($Check) {
    throw "cargo-deny $version is required. Run scripts/install-dependency-tools.ps1."
}

$arguments = @('install', 'cargo-deny', '--version', "=$version", '--locked', '--root', $tools)
if ($IsWindows) {
    & (Join-Path $root '.vscode/Invoke-Cargo.ps1') @arguments
} else {
    & cargo @arguments
}
if ($LASTEXITCODE -ne 0) {
    throw "cargo-deny $version installation failed with exit code $LASTEXITCODE."
}
$actual = & $binary --version
if ($LASTEXITCODE -ne 0 -or $actual -ne "cargo-deny $version") {
    throw "The installed checker does not match cargo-deny $version."
}
Write-Host "$actual is installed."
