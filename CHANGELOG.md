                                                                                                                            # Changelog

All notable changes to this project are documented in this file. The
format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Monorepo output splitting. When manifests are found in more than one
  directory, `squick scan` emits a navigation `context.md`, one focused
  `area-<name>.md` per detected sub-project (its stack, libraries, API
  surface, and notable files), and a cross-cutting `infra.md` for Docker /
  Compose. The global `conventions.md` and the `--full` graph stay whole, so
  cross-area references are not severed. Polyglot single-root projects stay
  single-file. New `--split auto|never` flag (default `auto`).
- ROI benchmark counts Dockerfiles and Compose files in the source corpus,
  and tolerates files that cannot be copied (Windows reserved names, broken
  symlinks) so it can measure real repositories.

## [2.0.0] - 2026-06-28

### Added

- Docker as a first-class stack. Dockerfile extraction covers base images,
  build stages, exposed ports, `ENTRYPOINT`/`CMD`, `WORKDIR`, `USER`, `ENV`
  keys, `ARG` names, and `VOLUME`. Compose extraction covers services,
  images, build contexts, ports, `depends_on`, `command`, `environment`
  keys, `env_file`, `volumes`, and `networks`. Environment values are
  dropped; only keys are kept to avoid leaking secrets into the context.
- Container semantic tags for runtime base images (`base-node`,
  `base-python`, `base-distroless`, ...), backing services
  (`service-postgres`, `service-redis`, ...), and multi-stage builds.
- `context.txt`: a compact columnar artifact for AI consumers, emitted with
  `--full`. Column names are declared once per record type, then rows are
  bare TAB-delimited values. Carries the same facts as `context.ndjson` at
  roughly 40% less size, with a larger token saving in practice.

### Changed

- Dictionaries are embedded in the `squick` binary at build time. An on-disk
  `dictionaries/` directory (or `SQUICK_DICT_DIR`, or one next to the
  executable) still overrides the embedded set for development.

### Fixed

- `cargo install squick-cli` produced a binary that could not locate its
  dictionaries, silently degrading framework detection. The embedded
  fallback resolves this so installed binaries are self-contained.

## [1.4.0] - 2026-06-15

### Added

- PHP support: Tree-sitter extraction of classes, traits, enums, methods,
  `use` imports, and call sites.
- `composer.json` manifest parsing with framework detection (Laravel,
  Symfony, Slim, CakePHP, Laminas, Yii, Drupal) and platform-requirement
  filtering (`php`, `ext-*`).
- Endpoint detection for Laravel route facades (`Route::get`), router-object
  routes (`$app->get`), and Symfony route attributes (`#[Route(...)]`).
- Laravel and Symfony pattern dictionaries under
  `dictionaries/frameworks/`.
- ROI benchmark (`benches/roi/`) measuring the context-compression ratio.
- CI workflow running `fmt`, `clippy`, and the test suite on Linux, macOS,
  and Windows.
- Supply-chain workflow: RUSTSEC advisory audit and a CycloneDX SBOM
  artifact, plus Dependabot for Cargo, npm, and GitHub Actions.
- End-to-end fixture tests asserting the scan-to-artifact pipeline.

### Changed

- Company name corrected to its full legal form, `Hub Horizon LLC`, across
  copyright headers, manifests, and documentation.
- `rust-version` corrected to `1.88`, the actual minimum imposed by
  transitive dependencies (was incorrectly declared `1.80`).

## [1.3.0] - 2026-05-20

### Added

- Status badges (npm, PyPI, crates.io, license) in the root README.
- `Built by Horizon LLC` section in the README with a link to the
  company site at <https://pixelhorizon.dev>.

### Changed

- README install section now describes the v1.2 three-file output
  (`conventions.md`, `schemas.md`, `context.md`) instead of the
  previous single-file model.
- `homepage` field across `Cargo.toml`, `package.json`, and
  `pyproject.toml` now points at <https://pixelhorizon.dev>; the
  `repository` field continues to point at GitHub.
- npm wrapper README and VS Code extension README aligned with the
  three-file story.

## [1.2.0] - 2026-05-20

### Added

- `squick scan --full` flag to emit `context.ndjson` and `graph.txt`
  alongside the chat-attachable artifacts.

### Changed

- `.squick/context.md` reduced to a tiny index that points at
  `conventions.md` (primary chat attachment) and `schemas.md`
  (data/API attachment).
- MCP `squick_scan` tool returns the conventions summary instead of
  the markdown overview, matching how agents actually consume context.
- Default scan no longer writes `context.ndjson` / `graph.txt`; they
  remain available via `--full` or the dedicated MCP tools.

## [1.1.0] - 2026-05-20

### Added

- `context.ndjson` emitter (one JSON fact per line) for programmatic
  LLM consumers.
- `graph.txt` emitter (subject-predicate-object triples) for graph
  traversal queries.
- `conventions.md` emitter that surfaces detected stack, library
  choices, repository layout, and API surface.
- MCP tools `squick_get_ndjson`, `squick_get_graph`, and
  `squick_get_conventions`.

### Changed

- `context.md` slimmed to a project-level summary plus a pointer to
  the other artifacts. Per-file symbol and JSX dumps moved out of
  the markdown view.

## [1.0.1] - 2026-05-13

### Fixed

- Excluded `bindings/node` and `bindings/python` from the workspace
  so cargo-dist no longer attempts to link the PyO3 bindings on the
  macOS release runner.

## [1.0.0] - 2026-05-13

Initial public release.

### Added

- Rust workspace with five library crates and the `squick` CLI.
- Tree-sitter-based extractor for TypeScript, TSX, JavaScript, JSX,
  and Python.
- Fourteen YAML pattern dictionaries covering Strapi, Next.js, Django,
  Django REST Framework, FastAPI, Flask, Express, Koa, Fastify,
  NestJS, Sanity, Payload, WordPress, React, and Tailwind.
- Endpoint detection for FastAPI/Flask decorators, Django
  urlpatterns, Express member-calls, and Next.js App Router file
  layout.
- `package.json` and `pyproject.toml` manifest parsing with framework
  inference.
- Strapi `schema.json` content-type extraction.
- Cross-file name-based reference resolver with ambiguity threshold.
- MCP server (`squick mcp`) on stdio with four tools.
- Distribution via npm (`@hubhorizonllc/squick`), PyPI (`squick`),
  crates.io (`squick-cli`), and prebuilt GitHub Release binaries
  (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64).

[1.4.0]: https://github.com/pwnaxe/squick/releases/tag/v1.4.0
[1.3.0]: https://github.com/pwnaxe/squick/releases/tag/v1.3.0
[1.2.0]: https://github.com/pwnaxe/squick/releases/tag/v1.2.0
[1.1.0]: https://github.com/pwnaxe/squick/releases/tag/v1.1.0
[1.0.1]: https://github.com/pwnaxe/squick/releases/tag/v1.0.1
[1.0.0]: https://github.com/pwnaxe/squick/releases/tag/v1.0.0
