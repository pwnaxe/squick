# Squick

Pre-computed, LLM-targeted code context for AI coding agents.

Squick scans a codebase, extracts structural facts (call graph, imports,
symbols, framework markers, HTTP endpoints, content-type schemas, route
patterns, dependency manifests), runs them through a tunable dictionary
of patterns, and emits a compact context file that AI agents read
instead of re-scanning the repository on every prompt.

## Why

AI coding agents currently spend tokens "looking around" the repository
before answering even simple questions. Squick inverts that cost: do
the analysis once at file save, save tokens on every prompt thereafter.

## Quick start

```bash
cargo run -p squick-cli --release -- scan ./your-project
```

This writes two files:

- `.squick/context.md` — primary, always read by the agent. Compact
  project map: structure, frameworks, files, symbols, references,
  endpoints.
- `.squick/schemas.md` — auxiliary, read on demand. Dependency
  manifests, full endpoint table, content-type schemas (Strapi,
  more coming).

## What gets extracted

- **Structure** (Tree-sitter): symbols, imports, JSX components, doc comments, references.
- **Heuristics**: function-name verbs, suffixes, Python dunders, framework markers.
- **Dictionaries** (YAML): conventional routes (`/about`, `/login`), file roles (`models.py`, `route.ts`), framework affinity.
- **Manifests**: `package.json` and `pyproject.toml` — project identity, dependencies, scripts, framework detection.
- **Endpoints**: FastAPI/Flask decorators, Django urlpatterns, Express member-calls, Next.js App Router file layout.
- **Data schemas**: Strapi content types (kind, names, attributes, relations).

## Supported languages

- TypeScript / TSX
- JavaScript / JSX
- Python

## Supported frameworks (out of the box)

Backend: Strapi, Django, Django REST Framework, FastAPI, Flask, Express,
Koa, Fastify, NestJS, Sanity, Payload CMS, WordPress (file roles).

Frontend: Next.js (App Router + Pages Router), React, Tailwind.

Add a YAML file under `dictionaries/frameworks/` to teach Squick a new
framework. No Rust changes required for most additions.

## CLI

```text
squick scan [root]                One-shot scan into .squick/context.md
  --format markdown|json          Output format (default: markdown)
  --out PATH                      Override output path
  --dict-dir PATH                 Override dictionary directory
  --include GLOB                  Repeatable. Only scan matching paths
  --exclude GLOB                  Repeatable. Skip matching paths
  --no-schemas                    Do not write .squick/schemas.md

squick watch [root]               Re-scan on file save (same flags)
squick init [root]                Create empty .squick/ directory
squick mcp                        Start an MCP server on stdio
  --dict-dir PATH                 Override dictionary directory
```

Examples:

```bash
# Only scan TypeScript and Python sources
squick scan --include '**/*.ts' --include '**/*.tsx' --include '**/*.py'

# Exclude tests and vendored code
squick scan --exclude 'tests/**' --exclude 'vendor/**'

# Skip auxiliary schemas file
squick scan --no-schemas
```

## MCP server (for AI agents)

Squick speaks the [Model Context Protocol](https://modelcontextprotocol.io)
on stdio. Any MCP-aware host can invoke its tools to pull project
context on demand rather than re-reading source files.

Tools exposed:

- `squick_scan(root)` — full project context as markdown.
- `squick_get_endpoints(root)` — HTTP endpoints as JSON (FastAPI,
  Flask, Django, Express, Next.js App Router).
- `squick_get_schemas(root)` — data schemas as JSON (Strapi content
  types and their attributes).
- `squick_get_file_context(root, file)` — context for one file only,
  cheaper than a full scan.

### Configure Claude Code

Add to your Claude Code config (`~/.claude/config.json` or equivalent):

```json
{
  "mcpServers": {
    "squick": {
      "command": "squick",
      "args": ["mcp"]
    }
  }
}
```

### Configure Cursor

Add to `.cursor/mcp.json` in your project (or global settings):

```json
{
  "mcpServers": {
    "squick": {
      "command": "squick",
      "args": ["mcp"]
    }
  }
}
```

### Configure Cline / Continue / other MCP clients

Most MCP clients accept the same `command` + `args` shape. Use
`squick` as the command (must be on `PATH`, or use the absolute path
to the binary) and `["mcp"]` as the args. Pass `--dict-dir
/path/to/dictionaries` if your dictionaries live outside the default
discovery locations.

### Manual smoke test

Pipe JSON-RPC over stdio to verify connectivity:

```bash
(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke-test","version":"1.0"}}}'
 echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
 echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}') | squick mcp
```

## Dictionary format

Dictionaries are YAML files under `dictionaries/<category>/<name>.yaml`:

```yaml
name: frameworks/example
description: One-line description of what this dictionary recognises.
entries:
  - pattern: "models.py"
    match: filename            # one of: route, component, filename,
                               #         path-segment, symbol-name, import
    tag: data-models           # label emitted on match
    confidence: high           # high | medium | low
    kind: literal              # literal | glob | regex (default: literal)
    note: "Optional context for reviewers."
```

Globs accept `*` and `?`. Regex uses Rust syntax. Literal matches are
case-insensitive.

## Workspace layout

```text
squick/
  crates/
    squick-core/       Types, scanner, AST extraction, resolver, manifests
    squick-dict/       YAML dictionary engine
    squick-format/     Markdown and JSON formatters
    squick-watch/      Debounced file watcher
    squick-cli/        `squick` binary
  bindings/
    node/              napi-rs bindings (npm distribution)
    python/            PyO3 bindings (PyPI distribution)
  extensions/
    vscode/            VS Code extension
  dictionaries/        YAML pattern catalogues
```

## License

Squick is distributed under the [Apache License 2.0](LICENSE).
Copyright 2026 Horizon LLC, Sharjah, United Arab Emirates.

## Trademarks

"Squick" and the Squick logo are trademarks of Horizon LLC. The Apache
License 2.0 grants no rights in the trademarks. See [TRADEMARKS.md](TRADEMARKS.md).

## Contributing

Contributions to source code, dictionaries, and documentation are
welcome under the terms of the Apache License 2.0. By submitting a
contribution, you agree that it is licensed under the same terms as
the project itself.

Run the test suite before submitting:

```bash
cargo test
```
