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
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

`--locked` makes Cargo fail if `Cargo.lock` is missing or dependency resolution
would change it. Keep this option for validation against the committed lockfile.

## Integration tests

Deterministic, reduced metadata fixtures live in `tests/fixtures/`. Each CLI
and MCP integration test creates its ROM folder under a temporary directory and
writes only small fake ROM files. The tests do not access the developer's
ArcadeJanitor cache or ROM collection.

Run the end-to-end test suites with:

```powershell
cargo test -p arcadejanitor-cli --test cli --locked
cargo test -p arcadejanitor-mcp --test http_integration --locked
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

## HTTP client compatibility evaluation

The evaluation for [#31](https://github.com/PlagueHO/arcade-janitor/issues/31)
preserves production HTTP, WebSocket, and stdio behavior. The endpoint is
custom stateless HTTP, not a standard Streamable HTTP implementation.

### Versions and official sources

The evaluation uses the official Rust SDK `rmcp` `3.8.0` from `Cargo.lock`.
It knows protocol revisions `2024-11-05`, `2025-03-26`, `2025-06-18`,
`2025-11-25`, and `2026-07-28`. Its latest version is `2026-07-28`.
Its default `ServiceExt::serve` still selects the initialization-based
lifecycle. Explicit `ClientLifecycleMode::Discover` selects modern discovery.
See the [official client lifecycle source](https://docs.rs/rmcp/3.8.0/src/rmcp/service/client.rs.html)
and [HTTP client documentation](https://docs.rs/rmcp/3.8.0/rmcp/transport/streamable_http_client/index.html).

The current official specification checked for this evaluation is
**`2026-07-28`**:

- [Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
  requires per-request protocol metadata and standard HTTP headers.
- [Versioning](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)
  replaces the initialization handshake with per-request version selection.
- [Authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization)
  defines optional OAuth-based HTTP authorization.

Compare these rules with the initialization-based
[Streamable HTTP revision `2025-11-25`](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
and the advertised
[`2024-11-05` HTTP+SSE transport](https://modelcontextprotocol.io/specification/2024-11-05/basic/transports).
Streamable HTTP first appeared in `2025-03-26`; an advertised `2024-11-05`
version does not identify a standard Streamable HTTP contract.

SSE responses and session IDs are optional in the initialization-era
Streamable HTTP binding. GET may return HTTP 405 when the server does not
offer an SSE stream. DELETE may return HTTP 405 for session termination.
Those responses alone are not compatibility failures. The `2026-07-28`
revision removed GET streams and protocol-level sessions.

### Measured results

Local execution on Windows produced these results. The normal regression
is included in the existing Linux, macOS, and Windows CI matrix; this
evaluation does not claim local execution on all three platforms.

| Check | Observed result | Meaning |
| --- | --- | --- |
| Default SDK initialization | Sends `initialize` with `2026-07-28`; accepts the server's `2024-11-05` result | The SDK's default initialization mode works with the custom endpoint. This is not modern version negotiation. |
| Tool discovery | Seven tools without a valid token; nine with the configured token | Custom token controls work through the official client. |
| Populated read-only call | `scan_roms` matches the exact shared fixture metadata and availability states | The SDK parses the tool result and returns usable data. |
| Initialized notification | POST returns HTTP 202 with no body | The initialization-mode notification succeeds. |
| Request headers | SDK sends both JSON and SSE in Accept and uses JSON Content-Type | The endpoint accepts the SDK's normal request format. |
| Subsequent version header | SDK sends `MCP-Protocol-Version: 2024-11-05` after initialization | The SDK follows the version returned by this server. |
| JSON responses | HTTP 200, JSON Content-Type, matching JSON-RPC request IDs | The SDK parses ordinary responses without an adapter. |
| Session and close | No session ID; default flow sends only POST; client close succeeds | No DELETE or server process termination is expected from a sessionless HTTP client close. |
| Missing or incorrect bearer token | Read-only calls succeed; destructive tools are hidden; attempted deletion returns a tool error and preserves files | No HTTP authentication challenge is issued. |
| Correct bearer token | SDK sends the header; destructive tools appear | Execution semantics remain covered by the existing safety tests. |
| Direct GET and DELETE probes | HTTP 405 | No SSE stream or session endpoint exists. This is not, by itself, a failure. |
| Modern SDK discovery | HTTP 200 with JSON-RPC `-32601: method not found` | `2026-07-28` discovery fails. Later modern SDK operations are blocked, not passed. |
| Unsupported or invalid version header | Read-only call still succeeds | The server ignores version headers instead of rejecting unsupported versions. |
| Inconsistent modern metadata | A body/header version mismatch, incorrect `Mcp-Method`, and incorrect `Mcp-Name` each still succeed | Required modern metadata validation is absent. |
| Untrusted HTTP Origin | Read-only call still succeeds | Standard HTTP Origin rejection is absent. Existing WebSocket Origin checks are unchanged. |
| OAuth discovery | Protected-resource metadata probes return HTTP 404; tool responses have no challenge | The custom token control is not OAuth interoperability. No live authorization service was used. |

The working SDK flow is **limited client interoperability**, not protocol
compliance. Modern discovery failure, ignored version and method metadata,
and missing HTTP Origin validation prevent a current Streamable HTTP claim.
The endpoint also does not provide the SSE endpoint required by its
advertised `2024-11-05` standard HTTP transport.

### Reproduce the evaluation

`sdk_http_client_reads_populated_custom_transport` runs in normal CI.
It uses the existing server process guard and populated `FlowFixture`.
A test-only HTTP observer forwards MCP body bytes and headers unchanged;
only Host changes to the upstream loopback address. It records requests
and responses without replacing the SDK transport or response parser.

The MCP crate enables `transport-streamable-http-client-reqwest` only as
a development feature. The `sdk-reqwest` development alias uses the SDK's
reqwest `0.13` backend, locked to `0.13.5`. Production code keeps the
workspace reqwest `0.12` dependency. The added client has no TLS features
because the evaluation uses loopback HTTP only.

Run the normal integration suite:

```powershell
cargo test -p arcadejanitor-mcp --test http_integration --locked
```

Result: **passed**, including the default SDK flow and raw contract probes.
The modern discovery reproducer is explicitly ignored by this command.
The existing workspace CI command includes the passing tests once per
platform; no extra CI test step is needed.

Run the unsupported modern flow explicitly:

```powershell
cargo test -p arcadejanitor-mcp --test http_integration --locked sdk_modern_http_discovery_compatibility -- --ignored --exact --nocapture
```

Result: **failed**, exit code 101, at modern discovery:

```text
modern SDK HTTP discovery failed: JSON-RPC error: -32601: method not found
```

The reproducer prints the actual SDK request, headers, HTTP status, JSON
response, and server diagnostics. It does not catch the failure and report
success. If modern support is added later, remove its ignore marker and
extend it to assert the full modern flow.

On Windows, prefix the same arguments with the Visual C++ wrapper:

```powershell
& .\.vscode\Invoke-Cargo.ps1 -CargoCommand test -CargoArgs @('-p', 'arcadejanitor-mcp', '--test', 'http_integration', '--locked')
& .\.vscode\Invoke-Cargo.ps1 -CargoCommand test -CargoArgs @('-p', 'arcadejanitor-mcp', '--test', 'http_integration', '--locked', 'sdk_modern_http_discovery_compatibility', '--', '--ignored', '--exact', '--nocapture')
```

Startup has a 10-second limit. HTTP requests, SDK operations, and client
close have 5-second limits. The three default token cases have a combined
60-second limit. All clients disable external proxies. Process guards stop
and reap the server on success and failure; the observer stops with its
test scope. Tests preserve all ROM bytes and unrelated files.

### Proposed separate behavior-change issue

**Proposed title:** Define and implement a standard MCP Streamable HTTP contract.

This proposal is not approval to change the server. Before implementation,
choose whether to support current `2026-07-28`, initialization-era revisions,
or both. Decide how to retain existing custom HTTP clients: keep their
contract on a separate route or approve an explicit migration. Decide
separately whether static token controls remain or OAuth support is required.

The follow-up should require:

- Official SDK discovery or initialization for each selected revision,
  exact populated tool results, applicable notifications, and bounded close.
- Required protocol metadata validation, correct unsupported-version and
  method errors, and HTTP Origin rejection.
- Clear authorization behavior without changing ROM safety or accepting
  agent-supplied ROM or metadata source paths.
- Existing custom HTTP, WebSocket, and stdio regression coverage during
  migration; release package checks against the agreed supported contract.
- Passing ordinary CI coverage for each supported SDK lifecycle and
  updated client installation guidance.

No follow-up issue is filed by this evaluation. The user must approve the
behavior-change scope first.

## Focused metadata tests

Run the focused metadata tests with:

```powershell
cargo test -p arcadejanitor-core --locked metadata::tests
```

CI runs `cargo test --workspace --locked` once on each platform in its Linux,
macOS, and Windows matrix. This command includes the core unit and integration
tests, both process suites, and library documentation tests. It includes all
metadata, CLI, and MCP flows added in
[#28](https://github.com/PlagueHO/arcade-janitor/issues/28). The focused commands
above are optional local checks, not extra CI steps. No tests require network
access outside the machine.

On Windows, use the Visual C++ wrapper if the current terminal has not loaded
the developer environment:

```powershell
& .\.vscode\Invoke-Cargo.ps1 -CargoCommand test -CargoArgs @('-p', 'arcadejanitor-cli', '--test', 'cli', '--locked')
& .\.vscode\Invoke-Cargo.ps1 -CargoCommand test -CargoArgs @('-p', 'arcadejanitor-mcp', '--test', 'http_integration', '--locked')
```

## Release package tests

The release workflow extracts each Linux, macOS, and Windows archive into a
new, empty temporary directory. It runs the extracted CLI and MCP server with
`--help`, checks exact CLI JSON against committed metadata, and starts the
sibling MCP server through the extracted CLI. A read-only `scan_roms` HTTP
tool call must return the expected metadata and availability for all fixture
entries and the unmatched archive.

The test uses the existing custom HTTP interface. It does not claim
Streamable HTTP compatibility. See
[#31](https://github.com/PlagueHO/arcade-janitor/issues/31) for that evaluation.
No AI client, MAME installation, or external network service is needed.

The test requires `ARCADEJANITOR_RELEASE_PACKAGE_DIR` to name an extracted
archive containing both executables. It uses absolute executable paths and
never falls back to build outputs or `PATH`. Missing executables fail the
test. The package test is ignored during normal workspace tests because
those tests do not create release archives. The release workflow selects
the package test explicitly before uploading the archive. A failure on any
platform blocks publication.

Build both release executables before creating a local archive:

```shell
cargo build -p arcadejanitor-cli -p arcadejanitor-mcp --release --locked
```

On Linux or macOS, create and extract a package with the same archive layout
as the release workflow:

```bash
(
set -euo pipefail
archive="$(mktemp -d)"
trap 'rm -rf -- "$archive"' EXIT
mkdir "$archive/package" "$archive/extracted"
cp target/release/arcadejanitor target/release/arcadejanitor-mcp "$archive/package/"
tar -czf "$archive/package.tar.gz" -C "$archive/package" arcadejanitor arcadejanitor-mcp
tar -xzf "$archive/package.tar.gz" -C "$archive/extracted"
export ARCADEJANITOR_RELEASE_PACKAGE_DIR="$archive/extracted"
cargo test -p arcadejanitor-mcp --test release_package --locked extracted_release_package -- --ignored --exact --nocapture
)
```

On Windows, use PowerShell:

```powershell
$ErrorActionPreference = 'Stop'
$archive = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path "$archive\package" | Out-Null
try {
    Copy-Item target\release\arcadejanitor.exe, target\release\arcadejanitor-mcp.exe -Destination "$archive\package"
    Compress-Archive -Path "$archive\package\*" -DestinationPath "$archive\package.zip"
    $env:ARCADEJANITOR_RELEASE_PACKAGE_DIR = "$archive\extracted"
    Expand-Archive -Path "$archive\package.zip" -DestinationPath $env:ARCADEJANITOR_RELEASE_PACKAGE_DIR
    & .\.vscode\Invoke-Cargo.ps1 -CargoCommand test -CargoArgs @('-p', 'arcadejanitor-mcp', '--test', 'release_package', '--locked', 'extracted_release_package', '--', '--ignored', '--exact', '--nocapture')
    if ($LASTEXITCODE -ne 0) { throw "Release package test failed: $LASTEXITCODE" }
} finally {
    if (Test-Path Env:\ARCADEJANITOR_RELEASE_PACKAGE_DIR) {
        Remove-Item Env:\ARCADEJANITOR_RELEASE_PACKAGE_DIR
    }
    Remove-Item -LiteralPath $archive -Recurse -Force
}
```

Do not repair executable permissions after extraction. The test must detect
incorrect permissions in the archive. All test ROMs, caches, configuration,
and process output use isolated temporary directories. Commands have a
15-second limit. Server startup has a 10-second limit. Incomplete health
responses are retried within this limit. HTTP requests and
each process cleanup wait have 5-second limits. Release test steps also have
a 15-minute outer limit.

The process guard stops the CLI and its server child before removing files.
It uses a separate
[Unix process group](https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#tymethod.process_group)
or a Windows
[job object](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).
The Windows-only `windows-sys` development dependency supplies the job APIs.
The launcher starts suspended, joins the job, and then resumes. This prevents
children from starting before the job owns them. Closing the job also stops
remaining children if cleanup fails. Unix cleanup uses
[`/bin/kill -KILL -- -PGID`](https://man7.org/linux/man-pages/man1/kill.1.html)
to keep the negative process group ID separate from options.
Only the test process tree is stopped, even after the launcher exits.
Regression tests check explicit cleanup, guard drop, and output collection
after launcher exit. The package test checks cleanup after
both a successful tool call and an induced failure after startup. It checks
that the listener is closed. Failures include captured process diagnostics.
Workflow cleanup removes the extracted package even when the test fails.

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
