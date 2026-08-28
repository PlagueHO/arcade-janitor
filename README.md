# CleanMAME

CleanMAME is a focused Rust CLI and MCP server for managing MAME ROM folders using only `mame.xml` and `catver.ini` metadata.

## Workspace

- `cleanmame-core` - shared models, parsers, operations, and utilities
- `cleanmame-cli` - command-line interface
- `cleanmame-mcp` - Axum HTTP/WebSocket MCP server

## v1 scope

CleanMAME v1 supports MAME ROM folders, `mame.xml`, `catver.ini`, metadata queries, filtering, moving, deleting, and simple reports. It intentionally excludes CHDs, BIOS dependency resolution, scraping, GUI, dashboards, cloud sync, plugins, and non-MAME systems.

`--catver` is optional. When it is omitted, CleanMAME downloads
[`catver.ini`](https://github.com/AntoPISA/MAME_SupportFiles/tree/main/catver.ini) from the
maintained upstream repository on first use and caches it in the current user's OS cache
directory. Supplying `--catver <path>` always uses that file instead.

When `--mame-executable <path>` is supplied without `--mame-xml`, CleanMAME runs
`mame.exe -listxml` and caches the result in the current user's OS cache directory.
The cached XML is reused when no XML path or executable is provided.

## Examples

```bash
cargo run -p cleanmame-cli -- scan --rom-folder ./roms --mame-xml ./mame.xml --catver ./catver.ini
cargo run -p cleanmame-cli -- query --name pacman --mame-xml ./mame.xml --catver ./catver.ini --json
cargo run -p cleanmame-cli -- filter --rom-folder ./roms --mame-xml ./mame.xml --catver ./catver.ini --genre maze
cargo run -p cleanmame-cli -- move --rom-folder ./roms --mame-xml ./mame.xml --catver ./catver.ini --genre maze --target-folder ./filtered --dry-run
cargo run -p cleanmame-cli -- delete --rom-folder ./roms --mame-xml ./mame.xml --catver ./catver.ini --genre mature --include-mature --dry-run
cargo run -p cleanmame-cli -- report --rom-folder ./roms --mame-xml ./mame.xml --catver ./catver.ini --json
```

Start the MCP server:

```bash
cargo run -p cleanmame-mcp
```

The server listens on `http://127.0.0.1:3000` by default. Keep it running in a
separate terminal while using an MCP client. Set `CLEANMAME_MCP_ADDR` to change
the listen address. Set `CLEANMAME_MCP_TOKEN` to enable the destructive tools
(`move_roms` and `delete_roms`).

### Install in VS Code

Build or run the MCP server from this repository:

```bash
cargo run -p cleanmame-mcp
```

Create `.vscode/mcp.json` in your workspace (or use **MCP: Open User
Configuration** from the Command Palette) with:

```json
{
  "servers": {
    "cleanmame": {
      "type": "http",
      "url": "http://127.0.0.1:3000/mcp"
    }
  }
}
```

Save the file, then use the MCP tools from GitHub Copilot Chat. If
`CLEANMAME_MCP_TOKEN` is set, add the matching bearer token to the
configuration's `headers` object:

```json
"headers": {
  "Authorization": "Bearer YOUR_TOKEN"
}
```

### Install in GitHub Copilot CLI

Start the server in a separate terminal:

```bash
cargo run -p cleanmame-mcp
```

Register its Streamable HTTP endpoint with Copilot CLI:

```bash
copilot mcp add --transport http cleanmame http://127.0.0.1:3000/mcp
```

The configuration is saved to `~/.copilot/mcp-config.json`. To use
destructive tools, include the token when registering the server:

```bash
copilot mcp add --transport http \
  --header "Authorization: Bearer YOUR_TOKEN" \
  cleanmame http://127.0.0.1:3000/mcp
```

Use `/mcp` in an interactive Copilot CLI session to verify that `cleanmame`
is connected and to view its available tools.

Endpoints:

- `GET /health`
- `POST /mcp` (MCP JSON-RPC)
- `GET /ws`

Set `CLEANMAME_MCP_TOKEN` to enable the destructive MCP tools; without it, those tools are
unavailable. Clients must authenticate using the standard HTTP authorization header. WebSocket
connections from non-local browser origins are rejected.

## Development

### Windows prerequisites

The Windows Rust toolchain in this project targets MSVC. Install the **Desktop
development with C++** workload from the Visual Studio Installer, including the
Windows SDK, then run Cargo from a **Developer PowerShell for Visual Studio**
terminal. This puts Microsoft's linker and its required libraries on `PATH`.

If the linker error reports `Usage: link FILE1 FILE2`, a different executable
named `link.exe` is being found first. Check the selected executable with:

```powershell
Get-Command link.exe -All
```

Run Cargo from the Visual Studio Developer PowerShell, or remove/reorder the
conflicting `link.exe` directory in your user/system `PATH`. For example, if
`C:\Program Files\coreutils\bin\link.exe` is returned, remove that directory
from `PATH` for the current Visual Studio Developer PowerShell session:

```powershell
$env:Path = ($env:Path -split ';' | Where-Object { $_ -ne 'C:\Program Files\coreutils\bin' }) -join ';'
```

The selected linker should be Visual Studio's `link.exe`, typically under
`Microsoft Visual Studio\...\VC\Tools\MSVC\...\bin\Hostx64\x64`.

Alternatively, use the repository's Dev Container, which provides a Linux Rust
toolchain and avoids host linker configuration.

### Dev Container

Open this repository in VS Code and run **Dev Containers: Reopen in Container** to
use the checked-in Rust development environment. It installs the stable Rust
toolchain with `rustfmt`, `clippy`, and `rust-src`, GitHub CLI, and the
workspace's recommended extensions. The MCP API is forwarded on port `3000`.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
