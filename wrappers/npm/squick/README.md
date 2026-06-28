# @hubhorizonllc/squick

[![npm](https://img.shields.io/npm/v/@hubhorizonllc/squick.svg?style=flat-square&logo=npm)](https://www.npmjs.com/package/@hubhorizonllc/squick)

Pre-computed, LLM-targeted code context for AI coding agents.

This npm package is a thin wrapper that installs the right
platform-specific binary via `optionalDependencies` and exposes it as
the `squick` command. The unscoped name `squick` was already taken
in 2015 by an unrelated, abandoned gulp/dustjs plugin, so the
Hub Horizon LLC distribution lives under the `@hubhorizonllc` scope. On
PyPI and crates.io the package is unscoped (`squick`, `squick-cli`).

## Install

```bash
npm i -g @hubhorizonllc/squick
# or, for one-off MCP usage (no install footprint):
npx -y @hubhorizonllc/squick mcp
```

After install, the binary is on `PATH` as plain `squick`.

## Usage

```bash
squick scan ./your-project       # one-shot scan
squick watch ./your-project      # re-scan on file save
squick mcp                       # start an MCP server on stdio
```

A scan writes three small files into `.squick/`:

- `conventions.md` - stack, libraries, layout. Attach to your AI chat.
- `schemas.md` - data schemas and endpoints. Attach for backend questions.
- `context.md` - tiny index pointing at the above.

Add `--full` to also emit `context.txt` and `context.ndjson` (programmatic
formats for MCP servers and scripts).

## Supported platforms

| OS      | Architecture | Package                                  |
| ------- | ------------ | ---------------------------------------- |
| Linux   | x86_64       | `@hubhorizonllc/squick-linux-x64`        |
| Linux   | aarch64      | `@hubhorizonllc/squick-linux-arm64`      |
| macOS   | x86_64       | `@hubhorizonllc/squick-darwin-x64`       |
| macOS   | arm64        | `@hubhorizonllc/squick-darwin-arm64`     |
| Windows | x86_64       | `@hubhorizonllc/squick-win32-x64`        |

## Built by Hub Horizon LLC

Squick is part of [pixelhorizon.dev](https://pixelhorizon.dev), the
developer-tools line from Hub Horizon LLC. We design and build custom AI
tooling, MCP integrations, and agent infrastructure for engineering teams.

## License

Apache-2.0. Copyright 2026 Hub Horizon LLC.
"Squick" is a trademark of Hub Horizon LLC.
