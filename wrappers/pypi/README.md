# squick (Python)

Pre-computed, LLM-targeted code context for AI coding agents.

This PyPI package ships the right platform-specific binary inside the
wheel and exposes it both as a `squick` console script and a small
Python API. Installation is platform-aware: `pip` picks the wheel for
your OS and architecture automatically.

## Install

```bash
pip install squick
```

## Use from the shell

```bash
squick scan ./your-project       # one-shot scan
squick watch ./your-project      # re-scan on file save
squick mcp                       # start an MCP server on stdio
python -m squick scan .          # equivalent to the above
```

## Use from Python

```python
import squick

# Run a scan and return the generated context.md as a string.
context = squick.scan("./your-project")
print(context)

# Locate the bundled binary, e.g. to spawn the MCP server manually.
print(squick.binary_path())
```

## Supported platforms

| OS      | Architecture | Wheel tag                  |
| ------- | ------------ | -------------------------- |
| Linux   | x86_64       | `manylinux2014_x86_64`     |
| Linux   | aarch64      | `manylinux2014_aarch64`    |
| macOS   | x86_64       | `macosx_10_12_x86_64`      |
| macOS   | arm64        | `macosx_11_0_arm64`        |
| Windows | x86_64       | `win_amd64`                |

See the [main project README](https://github.com/pwnaxe/squick) for
full documentation, dictionary format, and MCP client configuration.

## License

Apache-2.0. Copyright 2026 Hub Horizon LLC.
"Squick" is a trademark of Hub Horizon LLC.
