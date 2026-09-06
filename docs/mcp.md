# MCP Server Setup and Configuration

The MCP server manages exactly one ROM collection. It requires a ROM folder at
startup through `--rom-folder` or `ARCADEJANITOR_MAME_ROM_FOLDER`. MCP tools do
not accept a ROM folder or metadata-source paths, which keeps an agent bound to
the intended collection.

The server accepts `--mame-xml`, `--mame-executable`, and `--catver`, with the
same environment-variable equivalents as the [CLI](./installation.md#metadata-sources).

## Start the server

The standalone server uses HTTP by default:

```bash
./arcadejanitor-mcp --rom-folder ./roms --mame-xml ./mame.xml --catver ./catver.ini
```

Alternatively, start the packaged server through the CLI:

```bash
./arcadejanitor mcp start ./roms --mame-xml ./mame.xml --catver ./catver.ini
```

The HTTP server listens at `http://127.0.0.1:3000` by default. Set
`ARCADEJANITOR_MCP_ADDR` to choose another address. Its endpoints are:

- `GET /health`
- `POST /mcp` for MCP JSON-RPC
- `GET /ws`

Set `ARCADEJANITOR_MCP_TOKEN` to enable destructive tools such as moving and
deleting ROMs. HTTP clients must send the token as
`Authorization: Bearer <token>`.

## Install a stdio server

The CLI can register a user-scoped stdio server for VS Code, GitHub Copilot CLI,
or Claude Code:

```bash
./arcadejanitor mcp install vscode ./roms --mame-xml ./mame.xml --catver ./catver.ini
./arcadejanitor mcp install vscode-insiders ./roms --mame-xml ./mame.xml --catver ./catver.ini
./arcadejanitor mcp install copilot-cli ./roms --mame-executable /path/to/mame
./arcadejanitor mcp install claude-code ./roms
```

The target client command (`code`, `code-insiders`, `copilot`, or `claude`) must
be available on `PATH`. Re-running the command replaces only the existing
`arcadejanitor` server entry for that client.

## Install an HTTP server

Use `--transport http` to register an HTTP MCP endpoint. HTTP installation does
not require a local ROM folder, so it can register a server running on another
machine:

```bash
./arcadejanitor mcp install vscode --transport http \
  --url http://arcade-cabinet:3000/mcp
```

The HTTP server must be started separately on the machine containing the ROMs.
For a local server, provide its ROM folder and use `--start-now` to start it
after installation:

```bash
./arcadejanitor mcp install vscode ./roms --transport http --start-now
```

`--start-now` is only valid with HTTP transport. The server listens on
`http://127.0.0.1:3000` by default; set `ARCADEJANITOR_MCP_ADDR` before starting
the server when using another address, and set `--url` to the matching `/mcp`
endpoint.

## VS Code configuration

Create `.vscode/mcp.json` in a workspace, or use **MCP: Open User
Configuration** from the Command Palette:

```json
{
  "servers": {
    "arcadejanitor": {
      "type": "stdio",
      "command": "/absolute/path/to/arcadejanitor-mcp",
      "args": [
        "--transport", "stdio",
        "--rom-folder", "/absolute/path/to/roms",
        "--mame-xml", "/absolute/path/to/mame.xml",
        "--catver", "/absolute/path/to/catver.ini"
      ],
      "env": {
        "ARCADEJANITOR_MCP_TOKEN": "replace-with-a-secret-token"
      }
    }
  }
}
```

Use the corresponding environment variables in the `env` object instead of
command-line metadata settings when preferred.

## GitHub Copilot CLI configuration

Register a configured stdio server manually:

```bash
copilot mcp add --transport stdio arcadejanitor -- \
  /absolute/path/to/arcadejanitor-mcp --transport stdio \
  --rom-folder /absolute/path/to/roms --mame-xml /absolute/path/to/mame.xml \
  --catver /absolute/path/to/catver.ini
```

The configuration is stored in `~/.copilot/mcp-config.json`. For a manually
started HTTP server, register the `/mcp` endpoint and token header:

```bash
copilot mcp add --transport http \
  --header "Authorization: Bearer <token>" \
  arcadejanitor http://127.0.0.1:3000/mcp
```

Use `/mcp` in an interactive Copilot CLI session to confirm that
`arcadejanitor` is connected and inspect its available tools.
