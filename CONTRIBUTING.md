# Contributing to ArcadeJanitor

Thank you for contributing to ArcadeJanitor. Before opening a pull request, search
existing issues and use the appropriate issue form for bugs, feature requests, or
questions.

## Local setup

Install the stable Rust toolchain with the `rustfmt` and `clippy` components. The
included development container provides this toolchain and GitHub CLI. VS Code users
can accept the recommended extensions and run the workspace tasks from
**Terminal: Run Task**.

## Making changes

- Keep changes focused and avoid generated `target/` files.
- Preserve preview-by-default behavior for operations that move, delete, or clear
  user files; an explicit `--execute` flag must be required for mutation.
- Add or update tests when behavior changes, including integration tests for CLI or
  MCP behavior.
- Update `README.md` when user-facing usage changes.
- Add a concise user-facing entry under `## [Unreleased]` in `CHANGELOG.md` using
  the appropriate Keep a Changelog category.

## Dependency maintenance

Dependabot follows the shared `dependabot-monthly-v1` policy. GitHub Actions and
the root Cargo workspace are checked on the first day of each month at 09:00
`Pacific/Auckland`. Minor and patch updates are grouped as
`github-actions-minor-and-patch` and `cargo-minor-and-patch`; major updates remain
individual pull requests.

Security updates remain event-driven and are grouped separately as
`github-actions-security` and `cargo-security`; they are not deferred to the
monthly schedule or version-update cooldown. For an urgent routine update outside
the monthly window, request a manual check from the repository's Dependabot
interface. Manual checks can produce activity outside the normal schedule; the
configured cooldown and pull request limits still apply.

The current Dependabot scope does not cover the docs npm project or Dev Container
features. Keep those as explicit coverage gaps unless they are added in a separate
scope decision.

## Validation

Run the checks relevant to your change before opening a pull request:

```shell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

For focused integration validation, use one or more of:

```shell
cargo test -p arcadejanitor-core --test integration
cargo test -p arcadejanitor-cli --test cli
cargo test -p arcadejanitor-mcp --test http_integration
```

On Windows, the supplied VS Code MCP build and integration-test tasks initialize the
Visual C++ toolchain when necessary.

## Pull requests

Use the pull request template to describe the change, list validation you ran, and
confirm documentation and changelog updates. Keep pull requests reviewable by
separating unrelated changes.
