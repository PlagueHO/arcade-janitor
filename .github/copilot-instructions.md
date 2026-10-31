# Copilot instructions

## Project overview

ArcadeJanitor is a Rust 2024 workspace containing:

- `src/arcadejanitor-core` - shared models, parsers, operations, and utilities.
- `src/arcadejanitor-cli` - the `arcadejanitor` command-line interface.
- `src/arcadejanitor-mcp` - the MCP server and its HTTP/WebSocket transport.

Keep production changes scoped to the relevant crate. Reuse the core crate from the
CLI and MCP server rather than duplicating ROM, metadata, or output behavior.

## Safety and behavior

- ROM move, delete, and cache-clear operations must preview by default and require
  `--execute` before changing user files.
- The MCP server must keep its ROM folder and metadata sources server-configured;
  MCP tools must not accept these paths from an agent.
- Preserve the existing stdout-for-data and stderr-for-diagnostics convention so the
  CLI remains reliable in scripts.
- Do not modify generated `target/` content or unrelated files.

## Validation

Run the smallest relevant validation command before completing a change:

- Formatting: `cargo fmt --all -- --check`
- Linting: `cargo clippy --workspace --all-targets --locked -- -D warnings`
- Tests: `cargo test --workspace --locked`
- Targeted integration tests:
  - `cargo test -p arcadejanitor-core --test integration --locked`
  - `cargo test -p arcadejanitor-cli --test cli --locked`
  - `cargo test -p arcadejanitor-mcp --test http_integration --locked`

CI runs the workspace test command once per platform. It includes unit,
integration, end-to-end, and documentation tests. Use targeted commands only for
focused local validation, not as extra CI steps. Keep `--locked` on Cargo commands
that resolve dependencies so validation fails if the lockfile is missing or must
change. Do not add it to `cargo fmt`.

For VS Code on Windows, use the supplied MCP build and integration-test tasks when
the Visual C++ environment is required.

## Changelog requirement

Always update [`CHANGELOG.md`](../CHANGELOG.md) when making a project change.
Unless the change is a release itself, add a concise, user-facing entry under
the `## [Unreleased]` heading.

Use the [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/)
categories and only include categories that have entries:

- **Added** - new features
- **Changed** - changes to existing behavior
- **Deprecated** - features planned for removal
- **Removed** - removed features
- **Fixed** - bug fixes
- **Security** - security fixes

Review the changelog update as part of the implementation, and keep release
entries dated and linked with the version comparison references at the bottom
of the file.
