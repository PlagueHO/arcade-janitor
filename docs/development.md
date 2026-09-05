# Development Guide

ArcadeJanitor is a Rust workspace with a shared core crate, a CLI crate, and an
MCP server crate:

- `src/arcadejanitor-core` contains models, parsers, operations, and utilities.
- `src/arcadejanitor-cli` implements the command-line interface.
- `src/arcadejanitor-mcp` provides the HTTP, WebSocket, and stdio MCP server.

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
test starts the server on an ephemeral localhost port and calls `scan_roms`
through `POST /mcp`.

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
