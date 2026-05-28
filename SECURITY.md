# Security Policy

## Supported versions

| Version | Security fixes |
| ------- | -------------- |
| 1.3.x   | yes            |
| < 1.3   | no             |

Squick follows Semantic Versioning. Security fixes land on the
current minor release and ride along with the next patch.

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security problems.

Send a private report to <marketing@hubhorizon.tech> with:

- A description of the issue and the affected versions.
- A reproduction recipe or proof-of-concept, if available.
- Your assessment of impact.

You will receive an acknowledgement within five business days. We aim
to triage within ten business days, ship a fix in the next patch
release, and publish a public advisory once a fix is available.

## Scope

In scope:

- The `squick` CLI and its library crates.
- The official npm, PyPI, and crates.io packages distributed by
  Horizon LLC.
- The official GitHub Release artifacts.

Out of scope:

- Third-party forks, mirrors, or modified distributions.
- Vulnerabilities that require local code execution unrelated to
  Squick (compromised toolchain, malicious editor extensions, etc.).
- Misconfiguration of MCP hosts (Claude Code, Cursor, etc.).

## Coordinated disclosure

We disclose publicly only after a fix is available and users have a
reasonable window to upgrade. We credit reporters who request
acknowledgement.
