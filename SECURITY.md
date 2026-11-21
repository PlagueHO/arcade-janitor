# Security policy

## Report a vulnerability privately

Use this repository's
[private vulnerability reporting form](https://github.com/PlagueHO/arcade-janitor/security/advisories/new).
Private reporting is enabled. A GitHub account is required.
See GitHub's [reporting instructions](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/report-privately).

Do not put vulnerability details in a public issue or pull request.
If the private form is unavailable, open a public issue that asks for a private
reporting route. Do not include vulnerability details in that issue.

Include the affected version, operating system, expected result, observed
result, possible impact, and safe reproduction steps. Use synthetic ROM and
metadata files. Do not include credentials, tokens, private paths, or files
that you do not have permission to share. Do not test systems that you do not
control.

## Supported versions

Security fixes target the latest published release only. Older releases do
not receive backports. Upgrade to the latest release before reporting a
problem that might already be fixed.

The default branch is development code, not a supported release. Private
reports about development code are still welcome.

## Response and disclosure

Maintainers review reports through GitHub's private advisory tools.
There is no guaranteed response time, repair date, or support period.
A report does not promise a fix or a release.

Keep report details private while maintainers assess them. Use the private
thread to discuss impact, fixes, and coordinated disclosure. Do not put
sensitive evidence in public advisories or changelog entries.

## Dependency checks and limits

The repository checks Rust dependencies against RustSec advisories and an
explicit license policy. See the
[shared commands and limits](CONTRIBUTING.md#rust-dependency-policy).
The same policy runs locally and in CI.

Dependabot checks Cargo, documentation JavaScript, and GitHub Actions
dependencies. CodeQL checks source code. Secret scanning and push protection
check for exposed secrets, not dependency licenses. The Rust license check
does not cover JavaScript or action licenses.

These checks have different roles. No check proves that the project has no
vulnerabilities or that a dependency is safe in every use.
