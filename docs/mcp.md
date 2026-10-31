# MCP Server Setup and Configuration

The MCP server manages exactly one ROM collection. It requires a ROM folder at
startup through `--rom-folder` or `ARCADEJANITOR_MAME_ROM_FOLDER`. MCP tools do
not accept a ROM folder or metadata-source paths, which keeps an agent bound to
the intended collection.

Set `ARCADEJANITOR_CACHE_DIR` in the server environment to use a different
managed metadata cache directory. It contains `mame.xml` and `catver.ini`
directly. The default remains the system cache folder. This setting is not
an MCP tool argument and does not change explicit metadata paths.

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

## HTTP transport contract

`POST /mcp` is a **custom, stateless JSON-RPC transport**. It is not the
standard HTTP+SSE transport from MCP `2024-11-05`, and it does not claim
Streamable HTTP compliance.

Supported JSON-RPC requests receive one JSON response with HTTP 200 and
`Content-Type: application/json`. Notifications receive HTTP 202 with no
body. Calls do not require an initialization handshake. If a client sends
`initialize`, the server always reports `2024-11-05`; it does not negotiate
the requested version. `GET /mcp` and `DELETE /mcp` return HTTP 405.
There are no SSE streams, session IDs, or server-to-client HTTP notifications.

Read-only tools remain available without a token or with an incorrect token.
A correct bearer token enables destructive tools. An unauthorized
destructive call returns an MCP tool error, not an HTTP 401 challenge.
This is a static token control, not MCP OAuth authorization.

The official Rust `rmcp` `3.8.0` HTTP client completes its default
initialization-based flow: initialization, tool discovery, a populated
read-only call, the initialized notification, and client close. It accepts
the server's `2024-11-05` response. This measured client result does not
establish compliance with that revision or with current Streamable HTTP.

The same SDK's modern `2026-07-28` discovery mode fails at
`server/discover` with JSON-RPC error `-32601: method not found`.
The endpoint does not validate HTTP protocol-version metadata or HTTP
Origin headers. Do not expose it to untrusted networks on the assumption
that it implements the standard HTTP protections.

See the [HTTP compatibility evaluation](./development.md#http-client-compatibility-evaluation)
for the tested contract, specification references, and reproducible commands.
Use stdio when a client requires a standard supported connection.

## Install a stdio server

The CLI can register a user-scoped stdio server for VS Code, GitHub Copilot CLI,
or Claude Code:

```bash
./arcadejanitor mcp install --system vscode ./roms --mame-xml ./mame.xml --catver ./catver.ini
./arcadejanitor mcp install --system vscode-insiders ./roms --mame-xml ./mame.xml --catver ./catver.ini
./arcadejanitor mcp install --system copilot-cli ./roms --mame-executable /path/to/mame
./arcadejanitor mcp install --system claude-code ./roms
```

The target client command (`code`, `code-insiders`, `copilot`, or `claude`) must
be available on `PATH`. Re-running the command replaces only the existing
`arcadejanitor` server entry for that client.

## Install an HTTP server

HTTP registration writes a client configuration. It does not verify a
connection or make the endpoint Streamable HTTP compliant. The client
must support the custom contract described above.

Use `--transport http` to register an HTTP MCP endpoint. HTTP installation does
not require a local ROM folder, so it can register a server running on another
machine:

```bash
./arcadejanitor mcp install --system vscode --transport http \
  --url http://arcade-cabinet:3000/mcp
```

The HTTP server must be started separately on the machine containing the ROMs.
For a local server, provide its ROM folder and use `--start-now` to start it
after installation:

```bash
./arcadejanitor mcp install --system vscode ./roms --transport http --start-now
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
