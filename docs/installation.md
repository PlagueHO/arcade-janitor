# Installation

Download the package for your operating system from the
[latest GitHub release](https://github.com/PlagueHO/arcade-janitor/releases/latest).
Each package contains the `arcadejanitor` CLI and `arcadejanitor-mcp` server.

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
