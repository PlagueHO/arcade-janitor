#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][ValidatePattern('^v[0-9]+\.[0-9]+\.[0-9]+(?:-candidate\.[0-9a-f]{40})?$')][string]$Identity,
    [ValidateSet('linux', 'macos', 'windows')][string[]]$Platform = @('linux', 'macos', 'windows')
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$expected = @()
foreach ($item in $Platform) {
    $extension = if ($item -ceq 'windows') { 'zip' } else { 'tar.gz' }
    $expected += "arcadejanitor-$Identity-$item.$extension"
    $expected += "arcadejanitor-$Identity-$item.$extension.sha256"
}
$entries = @(Get-ChildItem -LiteralPath $Directory -Force)
if ($entries.Count -ne $expected.Count -or
    @($entries | Where-Object { $_.PSIsContainer -or ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) }).Count -or
    (Compare-Object -CaseSensitive $expected @($entries.Name))) {
    throw 'Release inventory must contain exactly one archive and checksum per platform.'
}
foreach ($item in $Platform) {
    $windows = $item -ceq 'windows'
    $extension = if ($windows) { 'zip' } else { 'tar.gz' }
    $name = "arcadejanitor-$Identity-$item.$extension"
    $archive = Join-Path $Directory $name
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    $checksum = [IO.File]::ReadAllText("$archive.sha256")
    if ($checksum -cnotmatch "^$hash  $([regex]::Escape($name))\r?\n$") {
        throw "Invalid checksum or changed archive: $name"
    }
    $suffix = if ($windows) { '.exe' } else { '' }
    $binaries = @("arcadejanitor$suffix", "arcadejanitor-mcp$suffix")
    if ($windows) {
        $zip = [IO.Compression.ZipFile]::OpenRead([IO.Path]::GetFullPath($archive))
        try {
            $members = @($zip.Entries.FullName)
            foreach ($entry in $zip.Entries) {
                $type = ($entry.ExternalAttributes -shr 16) -band 0xF000
                if ($entry.Length -eq 0 -or $type -notin @(0, 0x8000)) {
                    throw "Invalid archive member: $($entry.FullName)"
                }
            }
        } finally { $zip.Dispose() }
    } else {
        $members = @(& tar -tzf $archive)
        if ($LASTEXITCODE -ne 0) { throw "Cannot list archive: $name" }
        $details = @(& tar -tvzf $archive)
        if ($LASTEXITCODE -ne 0 -or @($details | Where-Object { $_ -cnotmatch '^-' }).Count) {
            throw "Archive must contain regular files only: $name"
        }
    }
    if ($members.Count -ne 2 -or (Compare-Object -CaseSensitive $binaries $members)) {
        throw "Archive must contain only the two root executables: $name"
    }
}
$expected | ForEach-Object { [IO.Path]::GetFullPath((Join-Path $Directory $_)) }
