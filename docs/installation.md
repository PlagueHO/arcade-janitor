# Installation

Download the package for your operating system from the
[latest GitHub release](https://github.com/PlagueHO/arcade-janitor/releases/latest).
Each package contains the `arcadejanitor` CLI and `arcadejanitor-mcp` server.

New packages prepared by the checked draft workflow include a `.sha256` sidecar.
Download the archive and its matching sidecar. Older releases might not supply
checksums or provenance. Do not claim those releases passed the new checks.

On Linux, verify the sidecar before extraction with `sha256sum -c <archive>.sha256`.
On macOS, use `shasum -a 256 -c <archive>.sha256`. On Windows, compare
`Get-FileHash -Algorithm SHA256 <archive>` with the digest in its sidecar.
A mismatch means the archive must not be used.

Checksums detect changed bytes but do not establish their origin. Use
[`gh attestation verify`](https://cli.github.com/manual/gh_attestation_verify)
to verify signed build provenance:

```shell
gh attestation verify <archive> --repo PlagueHO/arcade-janitor --signer-workflow PlagueHO/arcade-janitor/.github/workflows/release.yml --source-ref refs/tags/vX.Y.Z --source-digest <release-commit> --signer-digest <release-commit> --deny-self-hosted-runners
```

Replace the archive, version, and commit with the approved release identity.
Verify its checksum sidecar with the same command. For retained bundle verification,
add `--bundle <attestation.json>` from the release's provenance evidence artifact.
See the [release procedure](./development.md#release-preparation-and-publication).

## Linux and macOS

Extract the downloaded archive, using `macos` instead of `linux` for the macOS
package:

```bash
tar -xzf arcadejanitor-<version>-linux.tar.gz
chmod +x arcadejanitor arcadejanitor-mcp
./arcadejanitor --help
```

Move the extracted binaries to a directory on your `PATH` to use them from any
location.

## Windows

Extract the downloaded ZIP archive with PowerShell:

```powershell
Expand-Archive .\arcadejanitor-<version>-windows.zip -DestinationPath .\arcadejanitor
Set-Location .\arcadejanitor
.\arcadejanitor.exe --help
```

Move the extracted executables to a directory on `PATH` to run them from any
location.

## Metadata sources

ArcadeJanitor needs MAME XML metadata and optionally uses `catver.ini` for
category data. Pass local paths with `--mame-xml` and `--catver`, or use
`--mame-executable` to generate the MAME XML by running `mame.exe -listxml`.

When no path is provided, ArcadeJanitor downloads `catver.ini` on first use and
caches it in the current user's operating-system cache directory. Generated
MAME XML is cached there as well and reused when no XML path or executable is
provided.

The source options are global and may appear before or after a command:

```bash
./arcadejanitor rom list ./roms --mame-xml ./mame.xml --catver ./catver.ini
./arcadejanitor --mame-xml ./mame.xml rom list ./roms
```

Environment variables provide equivalent configuration:

| Setting | Environment variable |
| --- | --- |
| MAME XML file | `ARCADEJANITOR_MAME_XML` |
| MAME executable | `ARCADEJANITOR_MAME_EXECUTABLE` |
| catver.ini file | `ARCADEJANITOR_CATVER` |

See the [CLI reference](./cli.md) for everyday commands and the [MCP guide](./mcp.md)
to connect a configured collection to an AI client.
