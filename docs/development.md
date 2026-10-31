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
Destructive MCP tools preview changes by default. A caller must set `dry_run`
to `false` to apply a move or delete.

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

The CLI tests launch the compiled binary and parse JSON output. The MCP tests
check stateless HTTP calls, WebSocket messages, authorization, and populated
stdio flows through the official SDK client. Server diagnostics go to stderr;
stdout contains only data or stdio protocol messages.

Shared test support in `tests/support/` creates known available ROMs, a missing
ROM, an unmatched archive, and an unrelated text file. Both suites compare
normalized JSON records against the same exact expected metadata and
availability states. They also check filters, categories, and report counts.
CLI statistics count unmatched archives separately; MCP reports include them
in the available count. The tests preserve this difference.

Move and delete tests check default preview, explicit execution, and a fresh
scan. They compare archive bytes and check that unrelated files remain intact.
Collision tests preserve source and destination bytes. Authorization tests
attempt destructive calls against populated folders. CLI tests also check
`rom show`, audit findings, and audit exit codes at both failure thresholds.

Each child gets a temporary working directory and managed cache through
`ARCADEJANITOR_CACHE_DIR`. Tests remove inherited application settings and set
temporary home, configuration, and cache paths. The explicit managed cache
setting is required on Windows because its system cache lookup does not use
`LOCALAPPDATA` from the child environment. Tests do not change the test runner's
global environment.

The CLI metadata test compiles `tests/support/fake_mame.rs` with `rustc`. This
small native executable checks `-listxml`, records calls, and returns fixed
metadata or controlled failures. Tests check extraction, successful refresh,
cache reuse, failed-refresh preservation, and scoped cache-clear preview and
execution. No MAME installation is needed. Core download tests use bounded
local HTTP servers and disable proxy use in the test client. They check cache
replacement and preservation after HTTP or content errors.

CLI commands have a 15-second limit. The fake MAME compile has a 60-second
limit. MCP startup has a 10-second limit and requires both a successful health
status and the expected body. SDK calls, WebSocket reads/writes, and stdio
shutdown have 5-second limits. Captured diagnostics explain early server exits;
process guards stop and reap children after failures.

Run the focused metadata tests with:

```powershell
cargo test -p arcadejanitor-core metadata::tests
```

The existing CI workflow runs these tests through `cargo test --workspace`
and runs both process suites in its Linux, macOS, and Windows matrix. No tests
require network access outside the machine.

On Windows, use the Visual C++ wrapper if the current terminal has not loaded
the developer environment:

```powershell
& .\.vscode\Invoke-Cargo.ps1 -CargoCommand test -CargoArgs @('-p', 'arcadejanitor-cli', '--test', 'cli')
& .\.vscode\Invoke-Cargo.ps1 -CargoCommand test -CargoArgs @('-p', 'arcadejanitor-mcp', '--test', 'http_integration')
```

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
