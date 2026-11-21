#Requires -Version 7.0
[CmdletBinding()]
param(
    [string]$EventName = $env:GITHUB_EVENT_NAME,
    [string]$SourceRef = $env:GITHUB_REF,
    [string]$SourceCommit = $env:GITHUB_SHA,
    [string]$ManifestPath = (Join-Path (Split-Path -Parent $PSScriptRoot) 'Cargo.toml')
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$metadataText = & cargo metadata --manifest-path $ManifestPath --no-deps --format-version 1 --locked
if ($LASTEXITCODE -ne 0) { throw 'Cannot read locked Cargo metadata.' }
$metadata = $metadataText | ConvertFrom-Json
$versions = @($metadata.packages |
    Where-Object { $_.id -in $metadata.workspace_members } |
    Select-Object -ExpandProperty version -Unique)
if ($versions.Count -ne 1 -or $versions[0] -cnotmatch '^[0-9]+\.[0-9]+\.[0-9]+$') {
    throw 'All workspace versions must match one stable semantic version.'
}
if ($SourceCommit -cnotmatch '^[0-9a-f]{40}$') { throw 'Invalid source commit.' }
if ($SourceRef -clike 'refs/tags/*' -and $SourceRef -cne "refs/tags/v$($versions[0])") {
    throw 'The release tag must equal the workspace version.'
}
if ($EventName -ceq 'push') {
    if ($SourceRef -cne "refs/tags/v$($versions[0])") {
        throw 'The release tag must equal the workspace version.'
    }
    "v$($versions[0])"
} elseif ($EventName -cin @('pull_request', 'workflow_dispatch')) {
    "v$($versions[0])-candidate.$SourceCommit"
} else {
    throw "Unsupported package event: $EventName"
}
