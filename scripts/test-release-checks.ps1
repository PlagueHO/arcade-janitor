#Requires -Version 7.0
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$root = Join-Path ([IO.Path]::GetTempPath()) "arcadejanitor-release-checks-$([guid]::NewGuid())"
$identity = 'v0.1.0'
$checker = Join-Path $PSScriptRoot 'check-release-assets.ps1'
function Assert-Rejected {
    param([scriptblock]$Action, [string]$Message)
    $failure = $null
    try { & $Action | Out-Null } catch { $failure = $_.Exception.Message }
    if (-not $failure -or $failure -notlike "*$Message*") {
        throw "Expected rejection containing '$Message', got '$failure'."
    }
}
try {
    New-Item -ItemType Directory -Path $root, "$root/package", "$root/dist" | Out-Null
    foreach ($platform in @('linux', 'macos', 'windows')) {
        $suffix = if ($platform -ceq 'windows') { '.exe' } else { '' }
        $names = @("arcadejanitor$suffix", "arcadejanitor-mcp$suffix")
        Get-ChildItem -LiteralPath "$root/package" | Remove-Item
        foreach ($name in $names) { 'synthetic executable' | Set-Content -LiteralPath "$root/package/$name" }
        if ($platform -ceq 'windows') {
            $archive = "$root/dist/arcadejanitor-$identity-$platform.zip"
            Compress-Archive -Path "$root/package/*" -DestinationPath $archive
        } else {
            $archive = "$root/dist/arcadejanitor-$identity-$platform.tar.gz"
            & tar -czf $archive -C "$root/package" @names
            if ($LASTEXITCODE -ne 0) { throw 'Cannot create synthetic archive.' }
        }
        $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $([IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath "$archive.sha256" -Encoding ascii
    }
    $assets = @(& $checker -Directory "$root/dist" -Identity $identity)
    if ($assets.Count -ne 6) { throw 'Expected three archives and three checksums.' }
    Assert-Rejected { & $checker -Directory "$root/dist" -Identity 'v9.9.9' } 'inventory'
    'extra' | Set-Content "$root/dist/extra"
    Assert-Rejected { & $checker -Directory "$root/dist" -Identity $identity } 'inventory'
    Remove-Item -LiteralPath "$root/dist/extra"
    $checksum = "$root/dist/arcadejanitor-$identity-windows.zip.sha256"
    $original = [IO.File]::ReadAllText($checksum)
    Remove-Item -LiteralPath $checksum
    Assert-Rejected { & $checker -Directory "$root/dist" -Identity $identity } 'inventory'
    'malformed checksum' | Set-Content -LiteralPath $checksum
    Assert-Rejected { & $checker -Directory "$root/dist" -Identity $identity } 'checksum'
    [IO.File]::WriteAllText($checksum, $original)
    $archive = "$root/dist/arcadejanitor-$identity-windows.zip"
    $bytes = [IO.File]::ReadAllBytes($archive)
    $changed = $bytes.Clone()
    $changed[0] = $changed[0] -bxor 1
    [IO.File]::WriteAllBytes($archive, $changed)
    Assert-Rejected { & $checker -Directory "$root/dist" -Identity $identity } 'checksum'
    [IO.File]::WriteAllBytes($archive, $bytes)
    Remove-Item -LiteralPath $archive
    'extra executable' | Set-Content "$root/package/extra.exe"
    Compress-Archive -Path "$root/package/*" -DestinationPath $archive
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath $checksum -Encoding ascii
    Assert-Rejected { & $checker -Directory "$root/dist" -Identity $identity } 'two root executables'
    Remove-Item -LiteralPath "$root/package/extra.exe", "$root/package/arcadejanitor-mcp.exe", $archive
    Compress-Archive -Path "$root/package/*" -DestinationPath $archive
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $([IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath $checksum -Encoding ascii
    Assert-Rejected { & $checker -Directory "$root/dist" -Identity $identity } 'two root executables'
    Assert-Rejected {
        & (Join-Path $PSScriptRoot 'get-package-identity.ps1') -EventName push -SourceRef refs/tags/v9.9.9 -SourceCommit ('a' * 40)
    } 'workspace version'
    Assert-Rejected {
        & (Join-Path $PSScriptRoot 'verify-release-provenance.ps1') -Directory "$root/dist" -Identity $identity `
            -SourceCommit ('a' * 40) -SourceRef refs/tags/v0.1.0 -Bundle "$root/absent.json"
    } 'Missing attestation bundle'
    $identityScript = Join-Path $PSScriptRoot 'get-package-identity.ps1'
    New-Item -ItemType Directory -Path "$root/workspace", "$root/workspace/first", "$root/workspace/second" | Out-Null
    '[workspace]', 'members = ["first", "second"]', 'resolver = "2"' |
        Set-Content "$root/workspace/Cargo.toml"
    foreach ($member in @('first', 'second')) {
        '[package]', "name = `"$member`"", 'version = "0.1.0"', '[lib]', 'path = "lib.rs"' |
            Set-Content "$root/workspace/$member/Cargo.toml"
        '' | Set-Content "$root/workspace/$member/lib.rs"
    }
    & cargo generate-lockfile --manifest-path "$root/workspace/Cargo.toml" --offline
    if ($LASTEXITCODE -ne 0) { throw 'Cannot create the controlled workspace lockfile.' }
    $parameters = @{ ManifestPath = "$root/workspace/Cargo.toml"; SourceCommit = 'a' * 40 }
    $tag = & $identityScript @parameters -EventName push -SourceRef refs/tags/v0.1.0
    if ($tag -cne 'v0.1.0') { throw 'Valid version tag was rejected.' }
    foreach ($event in @('pull_request', 'workflow_dispatch')) {
        $candidate = & $identityScript @parameters -EventName $event -SourceRef refs/heads/main
        if ($candidate -cne "v0.1.0-candidate.$('a' * 40)") { throw 'Invalid candidate identity.' }
    }
    Assert-Rejected { & $identityScript @parameters -EventName push -SourceRef refs/tags/v0.1.0-beta } 'workspace version'
    Assert-Rejected { & $identityScript @parameters -EventName workflow_dispatch -SourceRef refs/tags/v9.9.9 } 'workspace version'
    Assert-Rejected { & $identityScript @parameters -EventName release -SourceRef refs/tags/v0.1.0 } 'Unsupported package event'
    '[package]', 'name = "second"', 'version = "0.2.0"', '[lib]', 'path = "lib.rs"' |
        Set-Content "$root/workspace/second/Cargo.toml"
    & cargo generate-lockfile --manifest-path "$root/workspace/Cargo.toml" --offline
    if ($LASTEXITCODE -ne 0) { throw 'Cannot update the controlled workspace lockfile.' }
    Assert-Rejected { & $identityScript @parameters -EventName push -SourceRef refs/tags/v0.1.0 } 'workspace versions must match'
    'Release inventory, checksum, member, version, and missing-provenance rejections passed.'
} finally {
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse }
}
$global:LASTEXITCODE = 0
