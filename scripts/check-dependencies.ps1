# Apply the same locked policy locally and in CI.
#Requires -Version 7.0
[CmdletBinding()]
param(
    [switch]$Offline,
    [string]$ManifestPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'Cargo.toml'),
    [string]$ConfigPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'deny.toml')
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
& (Join-Path $PSScriptRoot 'install-dependency-tools.ps1') -Check
$root = Split-Path -Parent $PSScriptRoot
$suffix = if ($IsWindows) { '.exe' } else { '' }
$binary = Join-Path $root ".dependency-tools/bin/cargo-deny$suffix"
$arguments = @(
    '--workspace', '--locked',
    '--manifest-path', [IO.Path]::GetFullPath($ManifestPath),
    '--config', [IO.Path]::GetFullPath($ConfigPath)
)
if ($Offline) {
    $arguments += '--offline'
}
$arguments += @('check', 'advisories', 'licenses')
& $binary @arguments
exit $LASTEXITCODE
