#!/usr/bin/env python3
# Copyright 2026 Hub Horizon LLC
# SPDX-License-Identifier: Apache-2.0
"""Measure the context-compression ratio Squick delivers.

An AI coding agent that does not use Squick reads source files to work out
how a project is laid out. Squick condenses that structural knowledge into
the `.squick/` artifacts. This script quantifies the difference: it counts
the tokens an agent would sift through in raw source versus the tokens in
the Squick artifacts it reads instead.

Token counts use OpenAI `tiktoken` (cl100k_base) when installed, otherwise a
chars/4 estimate. The same estimator is applied to both sides, so the ratio
is unbiased either way. Run with --help for options.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# Files an agent reads to understand structure: the languages Squick parses,
# the manifests it keys off, and the container files it analyzes. This is the
# corpus Squick replaces.
SOURCE_EXTS = {".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".py", ".php"}
MANIFEST_NAMES = {"package.json", "pyproject.toml", "composer.json", "schema.json"}


def is_container_file(name: str) -> bool:
    """Mirror of squick-core's Dockerfile/Compose detection."""
    low = name.lower()
    if low.endswith((".md", ".markdown", ".txt")):
        return False
    if low in {"dockerfile", "containerfile"}:
        return True
    if low.startswith("dockerfile.") or low.endswith(".dockerfile"):
        return True
    if low.endswith((".yml", ".yaml")) and (
        low.startswith("docker-compose") or low.startswith("compose.")
    ):
        return True
    return False

# Never counted or copied: build output, deps, VCS, prior artifacts.
SKIP_DIRS = {
    ".git", "node_modules", "target", "dist", "build", ".next",
    ".venv", "venv", "__pycache__", "vendor", ".squick", ".turbo",
}

ARTIFACTS = ("conventions.md", "schemas.md", "context.md")


def make_counter():
    """Return (count_fn, label). Prefers tiktoken, falls back to chars/4."""
    try:
        import tiktoken

        enc = tiktoken.get_encoding("cl100k_base")
        return (lambda s: len(enc.encode(s)), "tiktoken cl100k_base")
    except Exception:
        return (lambda s: (len(s) + 3) // 4, "chars/4 estimate")


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return ""


def iter_source_files(root: Path):
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.relative_to(root).parts):
            continue
        if (
            path.suffix in SOURCE_EXTS
            or path.name in MANIFEST_NAMES
            or is_container_file(path.name)
        ):
            yield path


def copy_clean(src: Path, dst: Path) -> None:
    """Copy a project into a temp dir, skipping heavy/irrelevant trees so a
    scan never mutates the real repo and never drags in target/ or deps.

    Tolerates files that cannot be copied (Windows reserved names such as
    `nul`, broken symlinks, permission errors); they are never source, so a
    partial copy is fine for measurement."""
    try:
        shutil.copytree(
            src,
            dst,
            ignore=shutil.ignore_patterns(*SKIP_DIRS),
            dirs_exist_ok=True,
            ignore_dangling_symlinks=True,
        )
    except shutil.Error as e:
        print(
            f"warning: skipped {len(e.args[0])} uncopyable file(s) in {src.name}",
            file=sys.stderr,
        )


def run_squick(binary: str, root: Path) -> None:
    subprocess.run(
        [binary, "scan", str(root)],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def measure_target(binary: str, target: Path, count) -> dict:
    with tempfile.TemporaryDirectory(prefix="squick-roi-") as tmp:
        work = Path(tmp) / target.name
        copy_clean(target, work)

        source_files = list(iter_source_files(work))
        source_tokens = sum(count(read_text(p)) for p in source_files)

        run_squick(binary, work)

        squick_dir = work / ".squick"
        squick_tokens = sum(
            count(read_text(squick_dir / name))
            for name in ARTIFACTS
            if (squick_dir / name).is_file()
        )

    reduction = (1 - squick_tokens / source_tokens) if source_tokens else 0.0
    return {
        "name": target.name,
        "files": len(source_files),
        "source_tokens": source_tokens,
        "squick_tokens": squick_tokens,
        "reduction": reduction,
    }


def resolve_binary(explicit: str | None) -> str:
    if explicit:
        return explicit
    for candidate in (
        Path("target/release/squick.exe"),
        Path("target/release/squick"),
        Path("target/debug/squick.exe"),
        Path("target/debug/squick"),
    ):
        if candidate.is_file():
            return str(candidate)
    found = shutil.which("squick")
    if found:
        return found
    sys.exit(
        "squick binary not found. Build it with `cargo build -p squick-cli` "
        "or pass --binary PATH."
    )


def render_table(rows: list[dict], estimator: str) -> str:
    out = [
        f"Token estimator: {estimator}",
        "",
        "| Project | Source files | Source tokens | Squick tokens | Reduction |",
        "| ------- | -----------: | ------------: | ------------: | --------: |",
    ]
    tot_files = tot_src = tot_sq = 0
    for r in rows:
        out.append(
            f"| {r['name']} | {r['files']} | {r['source_tokens']:,} | "
            f"{r['squick_tokens']:,} | {r['reduction'] * 100:.1f}% |"
        )
        tot_files += r["files"]
        tot_src += r["source_tokens"]
        tot_sq += r["squick_tokens"]
    tot_red = (1 - tot_sq / tot_src) if tot_src else 0.0
    out.append(
        f"| **Total** | **{tot_files}** | **{tot_src:,}** | "
        f"**{tot_sq:,}** | **{tot_red * 100:.1f}%** |"
    )
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "targets",
        nargs="*",
        help="Project directories to measure (default: bundled fixtures).",
    )
    parser.add_argument("--binary", help="Path to the squick binary.")
    parser.add_argument("--out", help="Write the markdown table to this file too.")
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[2]
    targets = (
        [Path(t).resolve() for t in args.targets]
        if args.targets
        else [repo_root / "fixtures" / "multi-framework", repo_root / "fixtures" / "sample"]
    )
    for t in targets:
        if not t.is_dir():
            sys.exit(f"not a directory: {t}")

    binary = resolve_binary(args.binary)
    count, estimator = make_counter()

    rows = [measure_target(binary, t, count) for t in targets]
    table = render_table(rows, estimator)
    print(table)
    if args.out:
        Path(args.out).write_text(table + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
