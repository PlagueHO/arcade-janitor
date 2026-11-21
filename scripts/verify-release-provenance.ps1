#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][string]$Identity,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-f]{40}$')][string]$SourceCommit,
    [Parameter(Mandatory)][ValidatePattern('^refs/(tags/v[0-9]+\.[0-9]+\.[0-9]+|heads/.+)$')][string]$SourceRef,
    [Parameter(Mandatory)][string]$Bundle
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
if (-not (Test-Path -LiteralPath $Bundle -PathType Leaf)) { throw 'Missing attestation bundle.' }
$assets = @(& (Join-Path $PSScriptRoot 'check-release-assets.ps1') -Directory $Directory -Identity $Identity)
foreach ($asset in $assets) {
    $arguments = @(
        'attestation', 'verify', $asset,
        '--repo', 'PlagueHO/arcade-janitor',
        '--signer-workflow', 'PlagueHO/arcade-janitor/.github/workflows/release.yml',
        '--signer-digest', $SourceCommit,
        '--source-digest', $SourceCommit, '--source-ref', $SourceRef,
        '--predicate-type', 'https://slsa.dev/provenance/v1',
        '--deny-self-hosted-runners'
    )
    & gh @arguments --bundle $Bundle
    if ($LASTEXITCODE -ne 0) { throw "Signed bundle verification failed: $asset" }
    & gh @arguments
    if ($LASTEXITCODE -ne 0) { throw "Current repository provenance verification failed: $asset" }
}
