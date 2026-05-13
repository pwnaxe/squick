# @hubhorizonllc/squick

Pre-computed, LLM-targeted code context for AI coding agents.

This npm package is a thin wrapper that installs the right
platform-specific binary via `optionalDependencies` and exposes it as
the `squick` command. The npm name `squick` was already taken in
2015 by an unrelated, abandoned dustjs/markdown plugin, so the
Horizon LLC distribution lives under the org scope. On PyPI and
crates.io the package is unscoped (`squick`) as expected.

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

See the [main project README](https://github.com/pwnaxe/squick)
for full documentation, dictionary format, and MCP client configuration.

## Supported platforms

| OS      | Architecture | Package                                  |
| ------- | ------------ | ---------------------------------------- |
| Linux   | x86_64       | `@hubhorizonllc/squick-linux-x64`        |
| Linux   | aarch64      | `@hubhorizonllc/squick-linux-arm64`      |
| macOS   | x86_64       | `@hubhorizonllc/squick-darwin-x64`       |
| macOS   | arm64        | `@hubhorizonllc/squick-darwin-arm64`     |
| Windows | x86_64       | `@hubhorizonllc/squick-win32-x64`        |

## License

Apache-2.0. Copyright 2026 Horizon LLC.
"Squick" is a trademark of Horizon LLC.
