# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added weekly Dependabot updates for GitHub Actions and Rust dependencies.
- Added CI, release, Rust toolchain, Rust edition, and license badges to the project README.
- Added the CleanMAME logo to the project README.
- Added `rom stats --show-unmatched` to list uncatalogued ROM archives.
- Added automated standalone Linux, macOS, and Windows release packages for version tags.
- Added this Keep a Changelog 1.1.0-formatted changelog.
- Added simple GitHub issue forms for bug reports, feature requests, and support questions.
- Added a pull request template with summary, testing, and review checklists.
- Added committed 100-entry integration fixtures and end-to-end CLI and MCP HTTP tests.
- Added VS Code tasks for running the CLI and MCP integration test suites together or separately.
- Added automatic extraction and OS cache reuse for MAME XML metadata.
- Added a modern, color-aware CLI help layout with a CleanMAME logo in interactive terminals.
- Improved the Windows MCP build task to initialize the Visual C++ toolchain automatically.
- Added selectable table, JSON, and TSV output formats to CLI commands.
- Added category and subcategory catalog views with case-insensitive filtering.
- Added an MCP tool for listing and querying catver categories, subcategories, and entries.
- Added MCP tools for showing and filtering MAME XML metadata without a ROM folder.
- Added ROM collection auditing, source cache management, and shell completion generation.
- Added TSV output, header control, sorting, and common catalog selectors.
- Added missing-ROM names to `rom stats` and repeatable category and subcategory filters.

### Changed

- Changed CI to run the core, CLI, and MCP integration test suites explicitly.
- Documented CleanMAME's focus on simple, agent-friendly ROM folder cleanup.
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

[unreleased]: https://github.com/PlagueHO/clean-mame/compare/main...HEAD
