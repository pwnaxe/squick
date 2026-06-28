# Squick

[![CI](https://github.com/pwnaxe/squick/actions/workflows/ci.yml/badge.svg)](https://github.com/pwnaxe/squick/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/squick-cli.svg?style=flat-square&logo=rust)](https://crates.io/crates/squick-cli)
[![npm](https://img.shields.io/npm/v/@hubhorizonllc/squick.svg?style=flat-square&logo=npm)](https://www.npmjs.com/package/@hubhorizonllc/squick)
[![PyPI](https://img.shields.io/pypi/v/squick.svg?style=flat-square&logo=pypi)](https://pypi.org/project/squick/)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache_2.0-blue.svg?style=flat-square)](LICENSE)

Pre-computed, LLM-targeted code context for AI coding agents.

Squick scans a codebase, extracts structural facts (call graph, imports,
symbols, framework markers, HTTP endpoints, content-type schemas, route
patterns, dependency manifests), and emits a small set of artifacts
that AI agents read instead of re-scanning the repository on every
prompt.

## Why

Every prompt to an AI coding agent starts the same way: the agent reads
through the repository to work out how it is laid out, then answers. That
exploration costs tokens and time on every prompt, including trivial ones.

Squick does the analysis once, on save, and writes the result to disk. The
agent reads that instead of re-deriving the project structure each turn.

## Measured impact

On a production Next.js + Python monorepo (863 source files, including its
Dockerfiles and Compose stack), the structural corpus an agent reads to
orient itself shrinks from ~1.87M tokens to ~4.9K:

| Layer | Source files | Source tokens | Squick tokens | Reduction |
| ----- | -----------: | ------------: | ------------: | --------: |
| Next.js frontend | 728 | 1,826,143 | 1,123 | 99.9% |
| Python backend | 135 | 41,045 | 3,791 | 90.8% |
| **Combined** | **863** | **1,867,188** | **4,914** | **99.7%** |

That is the recurring orientation tax Squick removes, paid on every prompt
otherwise. The benefit scales with repository size. Reproduce these numbers
on your own repo:

```bash
cargo build -p squick-cli
python benches/roi/measure.py /path/to/your/repo
```

Methodology and reference data: [benches/roi/](benches/roi/).

## Install

```bash
# npm (recommended for AI-agent users; works with `npx -y` too)
npm i -g @hubhorizonllc/squick

# PyPI
pip install squick

# crates.io
cargo install squick-cli

# Direct binary (Unix)
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/pwnaxe/squick/releases/latest/download/squick-cli-installer.sh | sh

# Direct binary (Windows)
irm https://github.com/pwnaxe/squick/releases/latest/download/squick-cli-installer.ps1 | iex
```

All channels install the same binary, exposed on `PATH` as `squick`.

## Quick start

```bash
squick scan ./your-project
```

Writes a small set of artifacts to `.squick/`:

- **`conventions.md`**: detected stack, library choices, repository
  layout, API surface. **Attach this to your AI chat** when asking
  about architecture or library usage.
- **`schemas.md`**: extracted data schemas (Strapi content types) and
  HTTP endpoints. **Attach this to your AI chat** for backend, data,
  or API questions.
- `context.md`: tiny index pointing at the two files above.

Each scan keeps `.squick/` in sync: artifacts Squick no longer emits (for
example an area file for a sub-project that moved) are removed, so the
directory always reflects the current scan. Files you add yourself are left
untouched.

### Monorepo output

When more than one top-level sub-project is detected (manifests in distinct
directories), Squick splits the output by area so an agent attaches only
the file relevant to its question:

- `context.md` becomes a **navigation** index routing each question to an area.
- `area-<name>.md` per detected sub-project: its stack, libraries, API
  surface, and notable files (e.g. `area-frontend.md`, `area-backend.md`).
  A manifest nested inside another (e.g. a plugin) folds into its parent
  area rather than becoming a peer.
- `area-other.md`: any files outside every sub-project, so nothing is dropped.
- `infra.md`: cross-cutting Docker / Compose configuration.
- `conventions.md` and the `--full` artifacts stay whole, so cross-area
  references are never severed.

Polyglot single-root projects (several manifests in one directory) stay
single-file. Force single output with `--split never`.

For programmatic consumers (MCP servers, scripts) add `--full`:

```bash
squick scan ./your-project --full
```

This additionally writes the tool-only AI artifacts:

- `context.txt` - compact columnar facts (densest, lowest token cost):
  one `@type` header per record kind, then TAB-delimited rows.
- `context.ndjson` - the same facts as JSON, one per line.

(RDF-style triples are still available on demand from the MCP
`squick_get_graph` tool.)

## What gets extracted

- **Structure** (Tree-sitter): symbols, imports, JSX components, doc comments, references.
- **Heuristics**: function-name verbs, suffixes, Python dunders, framework markers.
- **Dictionaries** (YAML): conventional routes, file roles, framework affinity.
- **Manifests**: `package.json`, `pyproject.toml`, `composer.json` (identity, dependencies, scripts, framework detection).
- **Endpoints**: FastAPI/Flask decorators, Django urlpatterns, Express member-calls, Next.js App Router file layout, Laravel route facades, Symfony route attributes.
- **Data schemas**: Strapi content types (kind, names, attributes, relations).
- **Containers**: `Dockerfile` base images, build stages, exposed ports; `docker-compose` services, images, ports, and `depends_on` links. Backing services (Postgres, Redis, etc.) and runtime base images surface as stack tags.

## Supported languages

TypeScript / TSX / JavaScript / JSX / Python / PHP.

## Supported frameworks (out of the box)

Backend: Strapi, Django, Django REST Framework, FastAPI, Flask, Express,
Koa, Fastify, NestJS, Sanity, Payload CMS, WordPress (file roles), Laravel,
Symfony.

Frontend: Next.js (App Router + Pages Router), React, Tailwind.

Infrastructure: Docker and Docker Compose (base images, build stages,
exposed ports, services, backing data stores).

Add a YAML file under `crates/squick-cli/dictionaries/frameworks/` to teach
Squick a new framework. No Rust changes required for most additions.

## CLI

```text
squick scan [root]                One-shot scan into .squick/
  --format markdown|json          Output format (default: markdown)
  --out PATH                      Override output path
  --dict-dir PATH                 Override dictionary directory
  --include GLOB                  Repeatable. Only scan matching paths
  --exclude GLOB                  Repeatable. Skip matching paths
  --no-schemas                    Skip .squick/schemas.md
  --full                          Also emit context.txt + context.ndjson
  --split auto|never              Split a monorepo per sub-project (default: auto)

squick watch [root]               Re-scan on file save (same flags)
squick init [root]                Create empty .squick/ directory
squick mcp                        Start an MCP server on stdio
  --dict-dir PATH                 Override dictionary directory
```

## MCP server (for AI agents)

Squick speaks the [Model Context Protocol](https://modelcontextprotocol.io)
on stdio. Any MCP-aware host can invoke its tools to pull project
context on demand rather than re-reading source files.

Tools exposed:

- `squick_scan(root)`: the conventions summary (most useful default).
- `squick_get_conventions(root)`: explicit conventions content.
- `squick_get_schemas(root)`: data schemas as JSON.
- `squick_get_endpoints(root)`: HTTP endpoints as JSON.
- `squick_get_file_context(root, file)`: context for one file only.
- `squick_get_ndjson(root)`: full project context as NDJSON.
- `squick_get_graph(root)`: RDF-style triples for graph traversal.

### Configure Claude Code

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

For zero-install invocation (no global package needed):

```json
{
  "mcpServers": {
    "squick": {
      "command": "npx",
      "args": ["-y", "@hubhorizonllc/squick", "mcp"]
    }
  }
}
```

The same shape works for Cursor (`.cursor/mcp.json`), Cline, Continue,
and any other MCP-aware host.

## Dictionary format

Dictionaries are YAML files under
`crates/squick-cli/dictionaries/<category>/<name>.yaml`:

```yaml
name: frameworks/example
description: One-line description of what this dictionary recognises.
entries:
  - pattern: "models.py"
    match: filename
    tag: data-models
    confidence: high
    kind: literal
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
    squick-format/     Output emitters (markdown / JSON / NDJSON / triples / conventions)
    squick-watch/      Debounced file watcher
    squick-cli/        `squick` binary
      dictionaries/    YAML pattern catalogues (embedded into the binary)
  bindings/
    node/              napi-rs bindings (npm distribution)
    python/            PyO3 bindings (PyPI distribution)
  extensions/
    vscode/            VS Code extension
```

## Built by Hub Horizon LLC

Squick is part of [**pixelhorizon.dev**](https://pixelhorizon.dev), the
developer-tools line from **Hub Horizon LLC**. We build tooling and MCP
servers for teams that work with AI coding agents.

Need something similar for your stack? Reach us at
[**pixelhorizon.dev**](https://pixelhorizon.dev).

## License

Squick is distributed under the [Apache License 2.0](LICENSE).
Copyright 2026 Hub Horizon LLC, Sharjah, United Arab Emirates.

## Trademarks

"Squick" and the Squick logo are trademarks of Hub Horizon LLC. The Apache
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
