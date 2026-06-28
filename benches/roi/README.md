# ROI benchmark

Quantifies the context-compression ratio Squick delivers: the tokens an AI
agent sifts through in raw source to orient itself, versus the tokens in the
`.squick/` artifacts it reads instead.

## Method

For each target project the script:

1. Copies the project into a temp directory, skipping `node_modules`,
   `target`, `vendor`, `.git`, build output, and any prior `.squick/`. The
   real repo is never touched.
2. Counts tokens across the source corpus an agent would read to learn the
   layout: every `.ts/.tsx/.js/.jsx/.mjs/.cjs/.py/.php` file, the manifests
   Squick keys off (`package.json`, `pyproject.toml`, `composer.json`,
   Strapi `schema.json`), and the container files it analyzes (`Dockerfile`,
   `Dockerfile.*`, `*.dockerfile`, `Containerfile`, `docker-compose*.yml`,
   `compose*.yml`).
3. Runs `squick scan` and counts tokens across the artifacts the agent
   actually reads: `conventions.md`, `schemas.md`, `context.md`.
4. Reports the reduction.

Token counts use OpenAI `tiktoken` (`cl100k_base`) when installed, otherwise
a chars/4 estimate. The same estimator runs on both sides, so the ratio is
unbiased regardless of which is used. Install `tiktoken` for exact counts:

```bash
pip install tiktoken
```

## Run

```bash
cargo build -p squick-cli
python benches/roi/measure.py                 # bundled fixtures
python benches/roi/measure.py /path/to/repo   # your own project
```

`--binary PATH` points at a specific build; `--out FILE` also writes the
table to disk.

## What the number means

The reduction is the recurring *orientation tax* Squick removes: the cost of
working out how a project is laid out, which an agent otherwise pays on every
prompt. It is not a claim that the agent never reads source again. It is the
size of the map versus the size of the territory.

The ratio scales with repository size. Tiny projects barely benefit, because
the fixed preamble in `conventions.md` is a large fraction of a small
corpus. Real codebases invert that completely.

## Reference results

A production Next.js + Python monorepo (863 source files), chars/4 estimate.
Squick 2.0.0 also condenses the repo's Dockerfiles and Compose stack into
`conventions.md`:

| Layer | Source files | Source tokens | Squick tokens | Reduction |
| ----- | -----------: | ------------: | ------------: | --------: |
| Next.js frontend | 728 | 1,826,143 | 1,123 | 99.9% |
| Python backend | 135 | 41,045 | 3,791 | 90.8% |
| **Combined** | **863** | **1,867,188** | **4,914** | **99.7%** |

The bundled fixtures (`fixtures/multi-framework`, `fixtures/sample`) are
deliberately tiny and dense. They exist to validate the scan-to-artifact
pipeline, not to demonstrate compression: below a few dozen files the fixed
preamble in `conventions.md` outweighs the source it summarizes, so the
ratio is flat or negative. Compression shows on real codebases, so run it on
one of yours:

```bash
python benches/roi/measure.py /path/to/your/repo
```
