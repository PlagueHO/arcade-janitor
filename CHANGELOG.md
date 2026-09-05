# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added a VitePress documentation website with installation, CLI, MCP server, and development guides.
- Added VS Code Insiders as an `mcp install` target using the `code-insiders` CLI.
- Added contributor guidance, shared VS Code Rust quality settings, and formatting
  and lint tasks.
- Added automated Rust setup steps for GitHub Copilot coding agent sessions.
- Added stdio MCP server support for direct VS Code and Copilot CLI configuration.
- Added `arcadejanitor mcp start` to launch the packaged MCP server for a ROM folder.
- Added `arcadejanitor mcp install` to configure the MCP server for VS Code, GitHub Copilot CLI,
  or Claude Code.
- Added integration coverage for metadata failures, ROM deletion and movement edge cases, and
  CLI error handling.

### Changed

- Displayed the supported systems directly in the `mcp install` command help.
- Updated installation and usage documentation for downloadable release binaries.
- Configured MCP servers now use a fixed required ROM folder and shared metadata source settings.
- Added MIT license information to the README.

### Fixed

- Fixed Windows MCP client installation when a supported CLI is exposed through a
  `.cmd` or `.bat` PATH launcher, including Unicode program names and actionable
  client-specific error messages.

## [0.1.0] - 2026-08-30

### Added

- Added weekly Dependabot updates for GitHub Actions and Rust dependencies.
- Added CI, release, Rust toolchain, Rust edition, and license badges to the project README.
- Added the ArcadeJanitor logo to the project README.
- Added `rom stats --show-unmatched` to list uncatalogued ROM archives.
- Added automated standalone Linux, macOS, and Windows release packages for version tags.
- Added this Keep a Changelog 1.1.0-formatted changelog.
- Added simple GitHub issue forms for bug reports, feature requests, and support questions.
- Added a pull request template with summary, testing, and review checklists.
- Added committed 100-entry integration fixtures and end-to-end CLI and MCP HTTP tests.
- Added VS Code tasks for running the CLI and MCP integration test suites together or separately.
- Added automatic extraction and OS cache reuse for MAME XML metadata.
- Added a modern, color-aware CLI help layout with a ArcadeJanitor logo in interactive terminals.
- Improved the Windows MCP build task to initialize the Visual C++ toolchain automatically.
- Added selectable table, JSON, and TSV output formats to CLI commands.
- Added category and subcategory catalog views with case-insensitive filtering.
- Added an MCP tool for listing and querying catver categories, subcategories, and entries.
- Added MCP tools for showing and filtering MAME XML metadata without a ROM folder.
- Added ROM collection auditing, source cache management, and shell completion generation.
- Added TSV output, header control, sorting, and common catalog selectors.
- Added missing-ROM names to `rom stats` and repeatable category and subcategory filters.

### Changed

- Changed the Cargo workspace layout to keep all application crates under `src/`.
- Changed the CLI startup banner to a four-color ASCII-art ArcadeJanitor logo.
- Renamed ArcadeJanitor to ArcadeJanitor.
- Changed CI to run the core, CLI, and MCP integration test suites explicitly.
- Documented ArcadeJanitor's focus on simple, agent-friendly ROM folder cleanup.
- Replaced the unpublished CLI with a consistent resource-first `rom`, `catalog`,
  `category`, and `source` command structure.
- Clarified `rom stats` category output with total, available, missing, and unmatched counts.
- Added comma-separated values for the repeatable `--category` selector, including
  PowerShell array-style input.
- Added category and subcategory columns to ROM move and delete output.
- Changed ROM move, delete, and cache clearing to preview by default and require
  `--execute` before modifying files.
- Changed metadata source and output controls into global options with environment-variable
  support.
- Standardized CLI output and diagnostics for reliable use in scripts.

### Fixed

- Fixed release publication when attaching platform packages.
- Fixed metadata downloads panicking when ROM commands run without an existing catver cache.
- Fixed CI and release workflows to use Node.js 24-compatible actions and avoid
  warnings from an unused Homebrew tap on macOS runners.
- Fixed source cache-clear previews to consistently report the requested clear action.
- Fixed CLI workspace builds after adding category data to operation output.
- Fixed `--include-mechanical` filtering so it can include non-runnable mechanical entries.
- Fixed move and delete output to report accurate source and destination paths and preview
  state.
- Fixed collection scans to distinguish uncatalogued archives from known non-runnable MAME
  entries.
- Fixed metadata refresh validation and deterministic category-entry output.

[unreleased]: https://github.com/PlagueHO/arcade-janitor/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/PlagueHO/arcade-janitor/releases/tag/v0.1.0
