# squick

Pre-computed, LLM-targeted code context for AI coding agents.

This npm package is a thin wrapper that installs the right
platform-specific binary via `optionalDependencies` and exposes it as
the `squick` command.

## Install

```bash
npm i -g squick
# or, for one-off MCP usage:
npx -y squick mcp
```

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
