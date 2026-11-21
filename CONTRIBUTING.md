# Contributing to ArcadeJanitor

Thank you for contributing to ArcadeJanitor. Before opening a pull request, search
existing issues and use the appropriate issue form for bugs, feature requests, or
questions.

For a suspected vulnerability, use the private reporting route in
[SECURITY.md](SECURITY.md). Do not post vulnerability details in a public issue.

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

Dependabot follows the shared `dependabot-monthly-v1` policy. GitHub Actions,
the root Cargo workspace, and the docs npm project are checked on the first day of each month at 09:00
`Pacific/Auckland`. Minor and patch updates are grouped as
`github-actions-minor-and-patch`, `cargo-minor-and-patch`, and
`docs-minor-and-patch`; major updates remain
individual pull requests.

Security updates remain event-driven and are grouped separately as
`github-actions-security`, `cargo-security`, and `docs-security`; they are not deferred to the
monthly schedule or version-update cooldown. For an urgent routine update outside
the monthly window, request a manual check from the repository's Dependabot
interface. Manual checks can produce activity outside the normal schedule; the
configured cooldown and pull request limits still apply.

The npm entry covers `docs/package.json` and `docs/pnpm-lock.yaml`.
Dev Container features remain outside this scope.
GitHub dependency alerts and Dependabot security updates must stay enabled.
Keep secret scanning and push protection enabled. They do not check licenses.

GitHub Actions use exact commit pins with release-version comments.
Update the pin and comment together. The docs pin pnpm in `docs/package.json`;
local commands and docs CI must use that version.

### Repository rulesets

The default branch uses the review model approved in
[PlagueHO/beyond-gate#39](https://github.com/PlagueHO/beyond-gate/issues/39).
Changes require a pull request, resolved review threads, and successful checks
against an up-to-date base. GitHub requires zero approving reviews, which lets
a sole maintainer merge their own checked PR. Human review and an explicit
maintainer merge request still apply. Administrators
cannot bypass the default-branch rules.

The required GitHub Actions checks are:

| Check | Purpose |
| --- | --- |
| `Rust (ubuntu-latest)` | Linux formatting, lint, tests, and build |
| `Rust (macos-latest)` | macOS formatting, lint, tests, and build |
| `Rust (windows-latest)` | Windows formatting, lint, tests, and build |
| `site` | Frozen docs dependency install and site build |
| `markdown` | Documentation style, local links, and controlled link tests |
| `dependencies` | Controlled policy tests, Rust advisories, and licenses |
| `Analyze (actions)` | GitHub Actions CodeQL analysis |
| `Analyze (javascript-typescript)` | VitePress TypeScript configuration and docs tooling |
| `Analyze (rust)` | Rust CodeQL analysis |

The user approved and activated these nine source check names in #40.
The two source `Synthetic native messaging (..., Windows)` contexts are excluded
because ArcadeJanitor has no browser native messaging product. No other source
check is omitted or replaced. Required jobs run on PRs and main pushes without
path filters. Pages deployment and package validation are not merge requirements.

Main ruleset `22311507` requires the nine checks above, bound to app `15368`
(`github-actions`). Successful PR and main runs verified their exact names before
separately approved activation. Before future required-check changes, verify
successful PR and main runs, then obtain approval of the exact live request.
Do not merge to bypass a pending check or silently retain nonexistent checks.

The temporary `Build Docs` and `dependency-policy` compatibility jobs were
removed after activation and effective-rule verification. The genuine `site`,
`markdown`, and `dependencies` jobs retain their checks.

CodeQL blocks errors and security alerts rated high or critical. Copilot reviews
non-draft pull requests on each push. The default branch cannot be deleted or
force-pushed. The approved policy does not use a separate `code_quality` rule.

Version tags matching `v*` have two active rulesets. Only repository
administrators can create them. No actor, including an administrator, can
update, delete, or force-push an existing version tag.
These settings live in GitHub, not in the workflow files.
See GitHub's [ruleset API reference](https://docs.github.com/en/rest/repos/rules).

The creation ruleset is `27719254`; the integrity ruleset is `27719237`.
Both target `refs/tags/v*` with no exclusions. Creation permits administrator
role `5` with bypass mode `always`. Integrity has explicit `update`, `deletion`,
and `non_fast_forward` rules and no bypass. Non-`v*` tags are outside this policy.
Keep these rules separate. The branch-only `update_allows_fetch_and_merge`
parameter is not required in a tag-rule response.

All other main parameters stay identical to the source: no bypass; active
default-branch targeting; strict checks; `do_not_enforce_on_create: true`;
zero approving reviews; resolved threads; no stale dismissal, Code Owner review,
last-push approval, required reviewers, or extra unattributed-change approval.
The ruleset allows merge, squash, and rebase. Repository settings remain
squash-only with auto-merge disabled. Do not widen them. Add no linear-history,
signed-commit, deployment, merge-queue, or unverified Code Quality requirement.

### Release approval and recovery

The release workflow prepares drafts only after package, checksum, existing-tag,
and signed provenance verification. PR builds are read-only. Manual runs never
create releases; only a manual run on the current default branch can attest
candidate assets. See the [release procedure](docs/development.md#release-preparation-and-publication).

Actions defaults stay read-only with PR-review approval disabled. Package builds
have `contents: read`. Only the separate provenance job has `actions: read`,
`id-token: write`, and `attestations: write` with `contents: read`. Only the
tag-triggered draft job has `contents: write`. No job publishes a release or
Cargo crate. Pages keeps its existing site and main-only deployment environment.

Future release immutability is enabled after separate approval and read-back
verification. Its REST enable operation is `PUT /repos/{owner}/{repo}/immutable-releases`
with no request body. See the [API reference](https://docs.github.com/rest/repos/repos#enable-immutable-releases).
Future setting changes need explicit approval of their exact live requests.
Issue creation, plan approval, or existing tag rules
do not supply this approval. Keep original, proposed, approved, and applied
settings separate. Save current settings and effective rules before any write.
Reread each setting immediately before its approved write. Stop on concurrent
changes, access denial, unsupported rules, or failed writes. Record successful
writes and partial application, then compare read-back with the approved fields.

Administrators can still edit rules. Users with write access can manage releases;
these controls do not make publication administrator-only. Once published with
immutability enabled, assets and the associated tag are locked. Titles, notes,
latest markers, and pre-release markers remain editable. Correct assets with a
new version. A rule edit cannot unlock an immutable release.

Emergency recovery needs approval for a narrow temporary rule edit. Record the
reason and original payload, restore the approved rule, and inspect it again.
Do not disable all protections or grant permanent bypass. Rollback also needs
approval and must not overwrite another administrator's changes. Configuration
inspection does not prove a rejected push, successful merge, or immutable
publication. Never merge, create/change a tag, or publish as a test.

### Rust dependency policy

Install PowerShell 7, Git, and Rust 1.95 or later. Windows installation also
requires Visual Studio C++ Build Tools. The installer uses the supplied
Visual C++ environment helper on Windows.

Run these commands from the repository root:

```powershell
pwsh -NoProfile -File ./scripts/install-dependency-tools.ps1
pwsh -NoProfile -File ./scripts/install-dependency-tools.ps1 -Check
pwsh -NoProfile -File ./scripts/check-dependencies.ps1
pwsh -NoProfile -File ./scripts/check-dependencies.ps1 -Offline
pwsh -NoProfile -File ./scripts/test-dependency-checks.ps1
```

The installer pins cargo-deny 0.22.0 with `cargo install --locked`.
It installs under `.dependency-tools/`, not the user's global Cargo bin
directory. Check commands reject a missing or different checker version.
Review the version pin and upstream release requirements before updating it.

`deny.toml` enables all features without a target filter. Checks include
production, build, test, optional, foreign-target, and unpublished workspace
dependencies. Cargo uses `--locked` so a check cannot silently resolve a new
dependency graph. Both local checks and CI run the same script and policy.

Normal checks fetch the current RustSec database and check registry indices
for yanked versions. A fetch failure fails the command. CI always uses this
online mode. A daily CI run also checks advisories published after code changes.

`-Offline` is explicit. It needs previously fetched dependency sources,
registry data, and advisory data. Missing data fails. Advisory data older
than seven days fails. This limit measures the database's last fetch, not
the date of its last advisory. Offline results cannot include advisories or
registry changes published after the cached data was fetched.

Advisory, unmaintained, unsound, and yanked dependency findings fail.
Licenses fail unless an approved SPDX expression can be satisfied.
The permissive license list applies to build and test dependencies as well
as production dependencies. No private-workspace exclusion is enabled.

Do not add a general advisory exclusion or a broad license allowance to
make CI pass. A maintainer must approve each exception first.
Record the exact dependency or advisory, reason, tracking issue, and expiry.
Use the checker's native expiry for advisory exceptions. Unused advisory
exceptions fail. Add expiry enforcement before introducing a license
exception; cargo-deny's license exception list does not provide that control.

Controlled tests use a temporary Cargo workspace and an invented advisory
database. They prove advisory and license rejection, full graph coverage,
exception expiry, offline freshness, tool failures, and exit-code propagation.
They also verify that checks preserve the fixture lockfile.
They do not use real ROM files or modify the production policy.

### Deferred dependency updates

The dependency update for [#39](https://github.com/PlagueHO/arcade-janitor/issues/39)
keeps these constraints explicit:

| Dependency | Retained version | Reason |
| --- | --- | --- |
| Production `reqwest` | 0.12.28 | 0.13.5 changes TLS feature names, the default crypto provider, and certificate verification defaults. Keep the existing Rustls/WebPKI trust behavior. The isolated SDK test client already uses 0.13.5 without TLS. See the [0.13 TLS configuration](https://docs.rs/reqwest/0.13.5/reqwest/tls/index.html). |
| `matchit` | 0.8.4 | The latest `axum` 0.8.9 requires exactly 0.8.4, so Cargo rejects 0.8.6. |
| `generic-array` | 0.14.7 | `crypto-common` 0.1.7 requires exactly 0.14.7 through Axum's SHA-1 dependency, so Cargo rejects 0.14.9. |
| VitePress | 2.0.0-alpha.21 | This is the newest VitePress 2 preview. The maintainer approved keeping compatible previews. Migration to beta and stable is tracked in [#42](https://github.com/PlagueHO/arcade-janitor/issues/42). |
| `chrono` | 0.4.45 | The CLI now uses `jiff` 1.0.1. `rmcp` 3.11.0 and `schemars` 1.2.3 still require `chrono`. [RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543.html) has one approved exception limited to these exact direct parent versions, expiring on 2026-12-20. Removal is tracked in [#41](https://github.com/PlagueHO/arcade-janitor/issues/41). |

The `chrono` finding is an unmaintained-package advisory, not evidence of an
exploitable vulnerability in ArcadeJanitor. Adopt a compatible released fix
from [modelcontextprotocol/rust-sdk#1474](https://github.com/modelcontextprotocol/rust-sdk/pull/1474),
then remove the exception and verify that `chrono` is absent from the full
dependency graph. Do not use an unreleased fork or disable other advisory checks.

The CLI uses `jiff` to format source modification times in the local time
zone. It keeps whole seconds and a numeric UTC offset, including daylight
saving changes and historical offset seconds. An unsupported timestamp or
failed time zone lookup makes the source detail unavailable with an error
in its `origin` field; it does not guess UTC.
See the [Jiff formatting rules](https://docs.rs/jiff/1.0.1/jiff/fmt/strtime/index.html)
and [system time zone lookup](https://docs.rs/jiff/1.0.1/jiff/tz/struct.TimeZone.html#method.try_system).

## Validation

Run the checks relevant to your change before opening a pull request:

```shell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

CI runs the workspace test command once on Linux, macOS, and Windows. It includes
unit, integration, end-to-end, and documentation tests. Do not repeat the focused
integration commands in CI. `--locked` makes Cargo fail if `Cargo.lock` is missing
or dependency resolution would change it.

For focused integration validation, use one or more of:

```shell
cargo test -p arcadejanitor-core --test integration --locked
cargo test -p arcadejanitor-cli --test cli --locked
cargo test -p arcadejanitor-mcp --test http_integration --locked
```

On Windows, the supplied VS Code MCP build and integration-test tasks initialize the
Visual C++ toolchain when necessary.

## Pull requests

Use the pull request template to describe the change, list validation you ran, and
confirm documentation and changelog updates. Keep pull requests reviewable by
separating unrelated changes.
