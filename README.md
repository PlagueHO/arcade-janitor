# CleanMAME

CleanMAME is a focused Rust CLI and MCP server for managing MAME ROM folders using only `mame.xml` and `catver.ini` metadata.

## Workspace

- `cleanmame-core` - shared models, parsers, operations, and utilities
- `cleanmame-cli` - command-line interface
- `cleanmame-mcp` - Axum HTTP/WebSocket MCP server

## v1 scope

CleanMAME v1 supports MAME ROM folders, `mame.xml`, `catver.ini`, metadata queries, filtering, moving, deleting, and simple reports. It intentionally excludes CHDs, BIOS dependency resolution, scraping, GUI, dashboards, cloud sync, plugins, and non-MAME systems.

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

Endpoints:

- `GET /health`
- `POST /mcp` (MCP JSON-RPC)
- `GET /ws`

Set `CLEANMAME_MCP_TOKEN` to enable the destructive MCP tools; without it, those tools are
unavailable. Clients must authenticate using the standard HTTP authorization header. WebSocket
connections from non-local browser origins are rejected.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
