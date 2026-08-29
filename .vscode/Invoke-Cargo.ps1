[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet("build", "test", "run", "package")]
    [string] $CargoCommand,

    [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
    [string[]] $CargoArgs
)

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $vswhere)) {
    throw "Visual Studio Installer's vswhere.exe was not found."
}

$installationPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath |
    Select-Object -First 1
if (-not $installationPath) {
    throw "A Visual Studio installation with C++ build tools was not found."
}

$vcvars = Join-Path $installationPath "VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path $vcvars)) {
    throw "Visual C++ environment script was not found: $vcvars"
}

$env:Path = "$(Split-Path $vswhere);$env:Path"
$arguments = @($CargoCommand) + @($CargoArgs | Where-Object { $null -ne $_ })
$quotedArguments = $arguments | ForEach-Object {
    '"' + $_.Replace('"', '\"') + '"'
}
$command = "call `"$vcvars`" && cargo $($quotedArguments -join ' ')"

& $env:ComSpec /d /s /c $command
exit $LASTEXITCODE
