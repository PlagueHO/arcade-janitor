#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Artifact,
    [Parameter(Mandatory)][string]$SourceCommit,
    [Parameter(Mandatory)][string]$SourceRef,
    [Parameter(Mandatory)][string]$Bundle
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$workflow = 'PlagueHO/arcade-janitor/.github/workflows/release.yml'
function Assert-Rejected {
    param([string]$File, [string]$Commit, [string]$Ref, [string]$Signer)
    & gh attestation verify $File --bundle $Bundle --repo PlagueHO/arcade-janitor `
        --source-digest $Commit --source-ref $Ref --signer-workflow $Signer `
        --predicate-type https://slsa.dev/provenance/v1 --deny-self-hosted-runners *> $null
    if ($LASTEXITCODE -eq 0) { throw 'Invalid artifact or identity passed signed provenance verification.' }
}
$changed = Join-Path ([IO.Path]::GetTempPath()) "arcadejanitor-tampered-$([guid]::NewGuid())"
try {
    Copy-Item -LiteralPath $Artifact -Destination $changed
    $stream = [IO.File]::Open($changed, [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite)
    try {
        $byte = $stream.ReadByte()
        if ($byte -lt 0) { throw 'Cannot test an empty artifact.' }
        $stream.Position = 0
        $stream.WriteByte($byte -bxor 1)
    } finally { $stream.Dispose() }
    Assert-Rejected $changed $SourceCommit $SourceRef $workflow
    $wrongCommit = if ($SourceCommit -ceq ('a' * 40)) { 'b' * 40 } else { 'a' * 40 }
    Assert-Rejected $Artifact $wrongCommit $SourceRef $workflow
    Assert-Rejected $Artifact $SourceCommit 'refs/heads/provenance-rejection' $workflow
    Assert-Rejected $Artifact $SourceCommit $SourceRef 'PlagueHO/arcade-janitor/.github/workflows/not-release.yml'
    & gh attestation verify $Artifact --bundle $Bundle --repo PlagueHO/arcade-janitor `
        --source-digest $SourceCommit --source-ref $SourceRef --signer-workflow $workflow `
        --predicate-type https://slsa.dev/provenance/v1 --deny-self-hosted-runners
    if ($LASTEXITCODE -ne 0) { throw 'Valid signed provenance failed after rejection checks.' }
} finally {
    if (Test-Path -LiteralPath $changed) { Remove-Item -LiteralPath $changed }
}
$global:LASTEXITCODE = 0
