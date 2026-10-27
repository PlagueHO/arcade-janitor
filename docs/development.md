# Development Guide

ArcadeJanitor is a Rust workspace with a shared core crate, a CLI crate, and an
MCP server crate:

- `src/arcadejanitor-core` contains models, parsers, operations, and utilities.
- `src/arcadejanitor-cli` implements the command-line interface.
- `src/arcadejanitor-mcp` provides the HTTP, WebSocket, and stdio MCP server.

The MCP server uses the [official Rust `rmcp` SDK](https://github.com/modelcontextprotocol/rust-sdk).
One tool handler serves all transports. Thin compatibility adapters keep the
existing stateless HTTP calls, WebSocket framing, and newline-delimited stdio
messages. The server continues to use MCP protocol version `2024-11-05`.
Keep tool schemas, bearer-token checks, Origin checks, and server-configured
ROM and metadata paths consistent across these adapters.

## Development environment

The included Dev Container provides a Linux Rust development environment with
the stable toolchain, `rustfmt`, `clippy`, `rust-src`, GitHub CLI, and the
recommended VS Code extensions. Open the repository in VS Code and select
**Dev Containers: Reopen in Container**.

Run the workspace quality checks from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Integration tests

Deterministic, reduced metadata fixtures live in `tests/fixtures/`. Each CLI
and MCP integration test creates its ROM folder under a temporary directory and
writes only small fake ROM files. The tests do not access the developer's
ArcadeJanitor cache or ROM collection.

Run the end-to-end test suites with:

```powershell
cargo test -p arcadejanitor-cli --test cli
cargo test -p arcadejanitor-mcp --test http_integration
```

The CLI test launches the compiled binary and verifies JSON output. The MCP
tests check stateless HTTP calls, WebSocket messages, authorization, and stdio
access from the official SDK client. Server diagnostics go to stderr; stdout
contains only data or stdio protocol messages.

## Documentation site

The site uses VitePress `2.0.0-alpha.21`. This is a prerelease, selected to remove
the vulnerable Vite 5 dependency chain without dependency overrides. Review the
[VitePress release notes](https://github.com/vuejs/vitepress/blob/main/CHANGELOG.md)
before the next upgrade. Check the build, navigation, images, theme, and code
highlighting after an upgrade.

Use Node.js 22.12 or later and pnpm 10, as used in docs CI. From `docs`, run:

```shell
npm exec --yes --package=pnpm@10 -- pnpm install --frozen-lockfile
npm exec --yes --package=pnpm@10 -- pnpm build
```

For local development or a preview of the built site, run one of:

```shell
npm exec --yes --package=pnpm@10 -- pnpm dev --host 127.0.0.1
npm exec --yes --package=pnpm@10 -- pnpm preview --host 127.0.0.1
```

Open the printed URL with the `/arcade-janitor/` base path. The build output
remains in `docs/dist`. Use pnpm to update `pnpm-lock.yaml`; do not edit it by hand.

## Windows prerequisites

The Windows Rust toolchain targets MSVC. Install the **Desktop development with
C++** workload from Visual Studio Installer, including the Windows SDK, and run
Cargo from a **Developer PowerShell for Visual Studio** terminal.

If a linker error reports `Usage: link FILE1 FILE2`, another `link.exe` is
earlier on `PATH`. Inspect the selected command with:

```powershell
Get-Command link.exe -All
```

Use the Visual Studio linker or remove the conflicting directory from the
current terminal's `PATH`. The expected executable is typically under
`Microsoft Visual Studio\...\VC\Tools\MSVC\...\bin\Hostx64\x64`.
