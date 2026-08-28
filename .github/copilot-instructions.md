# Copilot instructions

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
