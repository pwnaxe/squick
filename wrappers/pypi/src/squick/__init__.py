# Copyright 2026 Horizon LLC
# SPDX-License-Identifier: Apache-2.0
"""Squick CLI wrapper.

Bundles a platform-specific ``squick`` binary inside the wheel and exposes
it both as a ``squick`` console script and a thin Python API. The wheel
is platform-specific: ``pip`` picks the right one for the host.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path
from typing import Iterable

__all__ = ["main", "scan", "binary_path", "BinaryNotFoundError"]

_BINARY_NAME = "squick.exe" if sys.platform == "win32" else "squick"


class BinaryNotFoundError(RuntimeError):
    """Raised when the bundled squick binary cannot be located."""


def binary_path() -> str:
    """Return the absolute path to the bundled squick binary."""

    pkg_dir = Path(__file__).resolve().parent
    candidate = pkg_dir / "_binary" / _BINARY_NAME
    if candidate.is_file():
        return str(candidate)
    override = os.environ.get("SQUICK_BINARY")
    if override and Path(override).is_file():
        return override
    raise BinaryNotFoundError(
        f"squick binary not found at {candidate}. "
        "This usually indicates a broken installation; reinstall the package "
        "or set SQUICK_BINARY to point at a valid squick executable."
    )


def main() -> None:
    """Console-script entrypoint. Execs the binary with the caller's argv."""

    try:
        binary = binary_path()
    except BinaryNotFoundError as exc:
        print(f"squick: {exc}", file=sys.stderr)
        sys.exit(1)
    completed = subprocess.run([binary, *sys.argv[1:]], check=False)
    sys.exit(completed.returncode)


def scan(
    root: str | os.PathLike[str] = ".",
    *,
    extra_args: Iterable[str] | None = None,
    capture: bool = False,
) -> str:
    """Run ``squick scan`` against ``root`` and return the context markdown.

    By default the binary is invoked in pass-through mode so it writes
    ``.squick/context.md`` next to the project; the returned string is
    the contents of that file. Pass ``capture=True`` to return what the
    binary prints to stdout instead (useful when overriding ``--out -``).
    """

    binary = binary_path()
    args: list[str] = [binary, "scan", str(root)]
    if extra_args:
        args.extend(extra_args)
    if capture:
        result = subprocess.run(args, check=True, capture_output=True, text=True)
        return result.stdout
    subprocess.run(args, check=True)
    context_md = Path(root) / ".squick" / "context.md"
    return context_md.read_text(encoding="utf-8") if context_md.is_file() else ""
