# Security Policy

FinSight holds private financial data on the user's machine. We take security
reports seriously.

## Supported versions

| Version | Supported |
|---------|-----------|
| Latest release | Yes |
| Pre-release / main | Best effort |
| Older releases | No |

## Reporting a vulnerability

Email **security@finsight.app** (or, if not yet set up, the maintainer at the
email listed in `Cargo.toml`).

- Please include reproduction steps and the affected version.
- We aim to acknowledge reports within 3 business days and to ship a fix or
  mitigation within 30 days for confirmed high-severity issues.
- Do **not** open a public GitHub issue for security-sensitive reports.

## Scope

In-scope: the FinSight desktop app, its bundled SQLCipher database handling,
keychain integration, LLM provider integrations.

Out of scope: vulnerabilities in upstream dependencies (please report to their
maintainers), social-engineering attacks against the user's OS or bank.

## Disclosure

We follow coordinated disclosure: we publish the advisory once a fix is
released. Reporters who follow this policy are credited in the release notes
unless they prefer anonymity.
