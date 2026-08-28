# Repository guidance

## Changelog

Every change that affects the project must update [`CHANGELOG.md`](CHANGELOG.md)
in the same change set. Add entries under `## [Unreleased]` until a release is
made.

Use the categories defined by [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/):

- **Added** for new features.
- **Changed** for changes to existing functionality.
- **Deprecated** for features that will be removed.
- **Removed** for removed features.
- **Fixed** for bug fixes.
- **Security** for security-related changes.

Keep entries concise and written for users rather than implementation details.
Do not create empty categories. When releasing, move the unreleased entries to
a dated version heading and add the appropriate comparison link at the bottom
of the file.
