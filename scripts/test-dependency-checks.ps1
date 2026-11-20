# Exercise the real checker with isolated, synthetic policy failures.
#Requires -Version 7.0
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$root = Split-Path -Parent $PSScriptRoot
& (Join-Path $PSScriptRoot 'install-dependency-tools.ps1') -Check
$fixture = Join-Path ([IO.Path]::GetTempPath()) "ArcadeJanitor dependency checks $([guid]::NewGuid())"
$suffix = if ($IsWindows) { '.exe' } else { '' }
$manifest = Join-Path $fixture 'Cargo.toml'
$config = Join-Path $fixture 'deny.toml'
$lockfile = Join-Path $fixture 'Cargo.lock'
$checker = Join-Path $fixture 'scripts/check-dependencies.ps1'

function Assert-FixtureCheck {
    param([string]$Name, [int]$ExitCode, [string]$Diagnostic, [switch]$Online)

    $before = (Get-FileHash -LiteralPath $lockfile).Hash
    $arguments = @('-NoProfile', '-File', $checker, '-ManifestPath', $manifest, '-ConfigPath', $config)
    if (-not $Online) {
        $arguments += '-Offline'
    }
    $output = & pwsh @arguments 2>&1
    $actual = $LASTEXITCODE
    if ($before -ne (Get-FileHash -LiteralPath $lockfile).Hash) {
        throw "$Name changed Cargo.lock."
    }
    if ($actual -ne $ExitCode -or ($output -join "`n") -notmatch $Diagnostic) {
        $output | Out-Host
        throw "$Name returned $actual; expected $ExitCode and diagnostic '$Diagnostic'."
    }
    Write-Host "Passed: $Name."
}

try {
    foreach ($directory in @('scripts', '.dependency-tools/bin', 'src', 'platform/src', 'build/src', 'dev/src')) {
        New-Item -ItemType Directory -Path (Join-Path $fixture $directory) -Force | Out-Null
    }
    foreach ($name in @('check-dependencies.ps1', 'install-dependency-tools.ps1')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $fixture 'scripts')
    }
    $binary = Join-Path $fixture ".dependency-tools/bin/cargo-deny$suffix"
    Copy-Item -LiteralPath (Join-Path $root ".dependency-tools/bin/cargo-deny$suffix") -Destination $binary
    $serde = [regex]::Match(
        [IO.File]::ReadAllText((Join-Path $root 'Cargo.lock')),
        '(?m)^name = "serde"\r?\nversion = "([^"]+)"'
    )
    if (-not $serde.Success) {
        throw 'Cannot find the locked serde version for the offline fixture.'
    }
    $manifestText = @"
[package]
name = "policy-fixture"
version = "0.1.0"
edition = "2024"
license = "MIT"
publish = false

[workspace]
exclude = ["platform", "build", "dev"]

[features]
probe = ["dep:policy-platform"]

[target.'cfg(target_os = "none")'.dependencies]
policy-platform = { path = "platform", optional = true }

[build-dependencies]
policy-build = { path = "build" }

[dev-dependencies]
policy-dev = { path = "dev" }
serde = "=$($serde.Groups[1].Value)"
"@ + "`n"
    [IO.File]::WriteAllText($manifest, $manifestText)
    [IO.File]::WriteAllText((Join-Path $fixture 'src/lib.rs'), '')
    foreach ($name in @('platform', 'build', 'dev')) {
        [IO.File]::WriteAllText((Join-Path $fixture "$name/Cargo.toml"), @"
[package]
name = "policy-$name"
version = "0.1.0"
edition = "2024"
license = "MIT"
publish = false
"@ + "`n")
        [IO.File]::WriteAllText((Join-Path $fixture "$name/src/lib.rs"), '')
    }
    & cargo generate-lockfile --offline --manifest-path $manifest
    if ($LASTEXITCODE -ne 0) {
        throw 'Cannot prepare the offline fixture. Fetch the locked workspace dependencies first.'
    }

    $databaseRoot = Join-Path $fixture 'advisory databases'
    # This cache directory name belongs to the default RustSec URL in cargo-deny 0.22.0.
    $database = Join-Path $databaseRoot 'advisory-db-3157b0e258782691'
    New-Item -ItemType Directory -Path (Join-Path $database 'crates/serde') -Force | Out-Null
    & git init --quiet $database
    if ($LASTEXITCODE -ne 0) {
        throw 'Cannot prepare the synthetic advisory repository.'
    }
    $fetchHead = Join-Path $database '.git/FETCH_HEAD'
    [IO.File]::WriteAllText($fetchHead, '')
    $advisoryPath = Join-Path $database 'crates/serde/RUSTSEC-2000-9999.md'
    $advisory = @'
```toml
[advisory]
id = "RUSTSEC-2000-9999"
package = "serde"
date = "2000-01-01"
url = "https://example.invalid/synthetic-advisory"

[versions]
patched = [">=0.0.0"]
```

# Synthetic dependency policy test

This invented advisory is used only by an offline test.
'@ + "`n"
    [IO.File]::WriteAllText($advisoryPath, $advisory)
    $policy = [IO.File]::ReadAllText((Join-Path $root 'deny.toml'))
    $policy = [regex]::Replace($policy, '(?ms)^ignore = \[\r?\n.*?^\]', 'ignore = []')
    $policy = [regex]::Replace($policy, '(?m)^ignore = \[[^\r\n]*\]', 'ignore = []')
    $policy = $policy.Replace(
        '[advisories]',
        "[advisories]`ndb-path = '$databaseRoot'`ndisable-yank-checking = true"
    )
    [IO.File]::WriteAllText($config, $policy)
    Assert-FixtureCheck 'valid offline graph in a path with spaces' 0 'licenses ok'

    [IO.File]::WriteAllText($advisoryPath, $advisory.Replace('>=0.0.0', '>=999.0.0'))
    Assert-FixtureCheck 'advisory in a test-only dependency' 1 'error\[vulnerability\]'
    # Move serde from dev to build dependencies without changing its locked version.
    $buildManifest = $manifestText.Replace(
        "serde = `"=$($serde.Groups[1].Value)`"",
        ''
    ).Replace(
        '[build-dependencies]',
        "[build-dependencies]`nserde = `"=$($serde.Groups[1].Value)`""
    )
    [IO.File]::WriteAllText($manifest, $buildManifest)
    Assert-FixtureCheck 'advisory in a build-only dependency' 1 'error\[vulnerability\]'
    $foreignManifest = $manifestText.Replace(
        "serde = `"=$($serde.Groups[1].Value)`"",
        ''
    ).Replace(
        'probe = ["dep:policy-platform"]',
        'probe = ["dep:policy-platform", "dep:serde"]'
    ).Replace(
        "[target.'cfg(target_os = `"none`")'.dependencies]",
        "[target.'cfg(target_os = `"none`")'.dependencies]`nserde = { version = `"=$($serde.Groups[1].Value)`", optional = true }"
    )
    [IO.File]::WriteAllText($manifest, $foreignManifest)
    Assert-FixtureCheck 'advisory in an optional foreign-target dependency' 1 'error\[vulnerability\]'
    [IO.File]::WriteAllText($manifest, $manifestText)

    $tomorrow = [DateTime]::UtcNow.AddDays(1).ToString('yyyy-MM-dd')
    $active = 'ignore = [{ id = "RUSTSEC-2000-9999", reason = "Offline fixture only", expiry = "' + $tomorrow + '" }]'
    [IO.File]::WriteAllText($config, $policy.Replace('ignore = []', $active))
    Assert-FixtureCheck 'unexpired synthetic advisory exception' 0 'licenses ok'
    $scoped = $active.Replace(
        'reason = "Offline fixture only"',
        'reason = "Offline fixture only", allow = ["policy-fixture@0.1.0"]'
    )
    [IO.File]::WriteAllText($config, $policy.Replace('ignore = []', $scoped))
    Assert-FixtureCheck 'exception limited to an exact direct dependency parent' 0 'licenses ok'
    [IO.File]::WriteAllText($config, $policy.Replace(
        'ignore = []', $scoped.Replace('policy-fixture@0.1.0', 'policy-fixture@0.2.0')
    ))
    Assert-FixtureCheck 'exception rejects a different direct parent version' 1 'error\[vulnerability\]'
    $expired = $active.Replace($tomorrow, '2000-01-01')
    [IO.File]::WriteAllText($config, $policy.Replace('ignore = []', $expired))
    Assert-FixtureCheck 'expired advisory exception' 1 'error\[vulnerability\]'
    [IO.File]::WriteAllText($config, $policy.Replace('ignore = []', $active))
    [IO.File]::WriteAllText($advisoryPath, $advisory)
    Assert-FixtureCheck 'unused advisory exception' 1 'error\['
    [IO.File]::WriteAllText($config, $policy)

    foreach ($name in @('platform', 'build', 'dev')) {
        $dependencyManifest = Join-Path $fixture "$name/Cargo.toml"
        $original = [IO.File]::ReadAllText($dependencyManifest)
        [IO.File]::WriteAllText($dependencyManifest, $original.Replace('license = "MIT"', 'license = "GPL-3.0-only"'))
        Assert-FixtureCheck "disallowed license in $name dependency" 4 'error\[rejected\]'
        [IO.File]::WriteAllText($dependencyManifest, $original.Replace('license = "MIT"', ''))
        Assert-FixtureCheck "missing license in $name dependency" 4 'error\[unlicensed\]'
        [IO.File]::WriteAllText($dependencyManifest, $original)
    }
    $unlicensedRoot = $manifestText.Replace('license = "MIT"', '')
    [IO.File]::WriteAllText($manifest, $unlicensedRoot)
    Assert-FixtureCheck 'missing license in unpublished workspace crate' 4 'error\[unlicensed\]'
    [IO.File]::WriteAllText($manifest, $manifestText)

    (Get-Item -LiteralPath $fetchHead).LastWriteTimeUtc = [DateTime]::UtcNow.AddDays(-7).AddMinutes(-1)
    Assert-FixtureCheck 'database older than seven days' 1 'repository is stale'
    (Get-Item -LiteralPath $fetchHead).LastWriteTimeUtc = [DateTime]::UtcNow.AddDays(-7).AddMinutes(1)
    Assert-FixtureCheck 'database newer than seven days' 0 'licenses ok'
    [IO.File]::WriteAllText($config, $policy.Replace($databaseRoot, (Join-Path $fixture 'missing database')))
    Assert-FixtureCheck 'missing offline database' 1 'failed to'
    $missingRemote = ([Uri]::new((Join-Path $fixture 'missing remote'), [UriKind]::Absolute)).AbsoluteUri
    [IO.File]::WriteAllText($config, $policy.Replace(
        'https://github.com/RustSec/advisory-db', $missingRemote
    ))
    Assert-FixtureCheck 'online fetch failure does not use cached data' 1 'failed to' -Online
    [IO.File]::WriteAllText($config, $policy)

    $installer = Join-Path $fixture 'scripts/install-dependency-tools.ps1'
    $installText = [IO.File]::ReadAllText($installer)
    $pin = [regex]::Match($installText, '(?m)^\$version = ''[^'']+''\r?$')
    if (-not $pin.Success) {
        throw 'Cannot locate the checker version pin.'
    }
    [IO.File]::WriteAllText($installer, $installText.Replace($pin.Value, "`$version = '0.0.0'"))
    Assert-FixtureCheck 'wrong checker version' 1 'cargo-deny 0\.0\.0 is required'
    [IO.File]::WriteAllText($installer, $installText)
    Remove-Item -LiteralPath $binary
    Assert-FixtureCheck 'missing checker' 1 'cargo-deny .+ is required'
    Write-Host 'Dependency policy failure tests passed.'
}
finally {
    if (Test-Path -LiteralPath $fixture) {
        Remove-Item -LiteralPath $fixture -Recurse -Force
    }
}

# Reset the caller-visible native exit code only after assertions and cleanup succeed.
$global:LASTEXITCODE = 0
