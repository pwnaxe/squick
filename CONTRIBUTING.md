# Contributing to Squick

Thanks for considering a contribution. This document covers the rules,
the local workflow, and what we look for in incoming patches.

## Code of Conduct

By participating you agree to abide by the
[Code of Conduct](CODE_OF_CONDUCT.md).

## What we accept

| Contribution                              | Status                                       |
| ----------------------------------------- | -------------------------------------------- |
| Bug fixes with a test                     | always welcome                               |
| Performance improvements with a benchmark | always welcome                               |
| New YAML dictionaries                     | welcome; see `dictionaries/` for the format  |
| New language support (Tree-sitter)        | open an issue first to align scope           |
| New endpoint detectors                    | open an issue first to align scope           |
| Output format changes                     | open an issue first; downstream consumers care |
| Documentation                             | always welcome                               |

For anything that changes a public API (CLI flags, MCP tool surface,
output file format), please open an issue before sending a PR so we
can agree on the shape.

## Local workflow

```bash
# Install the Rust toolchain (1.80+) and check out the repository.
git clone https://github.com/pwnaxe/squick.git
cd squick

# Build and test the library crates and the CLI.
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check

# Smoke-test the binary on a sample project.
cargo run -p squick-cli --release -- scan ./fixtures/multi-framework
```

The `bindings/` crates are excluded from the workspace because they
require platform-specific toolchains (napi-rs, maturin / PyO3). Build
them separately if you are working on the npm or PyPI wrapper.

## Pull request checklist

- The PR description references the issue it resolves, if any.
- `cargo fmt --check` and `cargo clippy ... -D warnings` are clean.
- New behaviour is covered by a test.
- Public APIs and configuration changes are documented.
- The `CHANGELOG.md` is updated under the `Unreleased` heading.

## Adding a framework dictionary

YAML lives in `dictionaries/<category>/<name>.yaml`. The schema is
documented in the project README. A new dictionary needs at minimum:

- A `name` that matches its directory and filename.
- A one-line `description` of what it recognises.
- A list of `entries`, each with a `pattern`, `match` surface, target
  `tag`, and a `confidence` rating.

Open a PR with the YAML file plus a fixture (or test case) showing
the dictionary firing on representative code. Pure-data additions
do not require Rust changes.

## License of contributions

Squick is distributed under the
[Apache License 2.0](LICENSE). By submitting a contribution you agree
that it is licensed under the same terms.

## Reporting security issues

Please do not file public issues for security problems. Follow the
process in [SECURITY.md](SECURITY.md).
