# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added this Keep a Changelog 1.1.0-formatted changelog.
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

- Replaced the unpublished CLI with a consistent resource-first `rom`, `catalog`,
  `category`, and `source` command structure.
- Changed ROM move, delete, and cache clearing to preview by default and require
  `--execute` before modifying files.
- Changed metadata source and output controls into global options with environment-variable
  support.
- Standardized CLI output and diagnostics for reliable use in scripts.

### Fixed

- Fixed `--include-mechanical` filtering so it can include non-runnable mechanical entries.
- Fixed move and delete output to report accurate source and destination paths and preview
  state.
- Fixed collection scans to distinguish uncatalogued archives from known non-runnable MAME
  entries.
- Fixed metadata refresh validation and deterministic category-entry output.

[unreleased]: https://github.com/PlagueHO/clean-mame/compare/main...HEAD
