# Squick for VS Code

Pre-computed LLM context for AI agents inside VS Code.

## Features

- `Squick: Scan workspace` - one-shot scan; writes `.squick/conventions.md`
  (primary) and `.squick/schemas.md` (data schemas / endpoints) alongside a
  small `context.md` index.
- `Squick: Toggle watch mode` - live rescan on file save.
- `@squick` chat participant - answers Copilot Chat questions with the
  bundled project context.

## Status

Skeleton stage. Activates and registers commands; scanner integration is in
progress.

## Built by Hub Horizon LLC

Squick is part of [pixelhorizon.dev](https://pixelhorizon.dev), the
developer-tools line from Hub Horizon LLC. Custom AI tooling for
engineering teams.

Licensed under the Apache License 2.0. Copyright 2026 Hub Horizon LLC.
