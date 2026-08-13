#!/usr/bin/env python3
"""Run release gates and write a content-bound, fail-closed verification receipt."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]


def sha256(path: Path) -> str | None:
    if not path.is_file():
        return None
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def capture(command: list[str], timeout: int) -> dict[str, Any]:
    executable = command[0]
    if shutil.which(executable) is None:
        return {
            "command": command,
            "status": "unavailable",
            "exit_code": None,
            "output_tail": f"{executable} was not found",
        }
    started = dt.datetime.now(dt.timezone.utc)
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=timeout,
            check=False,
            env={**os.environ, "CARGO_TERM_COLOR": "never"},
        )
    except subprocess.TimeoutExpired as error:
        output = error.stdout or ""
        if isinstance(output, bytes):
            output = output.decode("utf-8", errors="replace")
        return {
            "command": command,
            "status": "timed_out",
            "exit_code": None,
            "elapsed_seconds": timeout,
            "output_tail": output[-8000:],
        }
    elapsed = (dt.datetime.now(dt.timezone.utc) - started).total_seconds()
    return {
        "command": command,
        "status": "passed" if completed.returncode == 0 else "failed",
        "exit_code": completed.returncode,
        "elapsed_seconds": elapsed,
        "output_tail": completed.stdout[-8000:],
    }


def version(command: list[str]) -> str | None:
    result = capture(command, 30)
    if result["status"] != "passed":
        return None
    return str(result["output_tail"]).strip().splitlines()[0]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=1800)
    parser.add_argument(
        "--include-wasm",
        action="store_true",
        help="also rebuild the browser WebAssembly artifact",
    )
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")

    commands: list[tuple[str, list[str]]] = [
        ("rustfmt", ["cargo", "fmt", "--all", "--", "--check"]),
        (
            "clippy",
            [
                "cargo",
                "clippy",
                "--workspace",
                "--all-targets",
                "--no-default-features",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("rust_tests", ["cargo", "test", "--workspace", "--no-default-features"]),
        ("repository_contract", [".venv/bin/python", "scripts/check_repo.py"]),
        ("simulations", [".venv/bin/python", "scripts/generate_simulations.py"]),
        (
            "paper",
            [
                "latexmk",
                "-cd",
                "-pdf",
                "-interaction=nonstopmode",
                "-halt-on-error",
                "paper/main.tex",
            ],
        ),
    ]
    if args.include_wasm:
        commands.append(("wasm", ["bash", "scripts/build_wasm.sh"]))

    results = {name: capture(command, args.timeout) for name, command in commands}
    git_head = capture(["git", "rev-parse", "HEAD"], 30)
    git_status = capture(["git", "status", "--porcelain=v1"], 30)
    source_commit = (
        git_head["output_tail"].strip() if git_head["status"] == "passed" else None
    )
    dirty = bool(git_status["output_tail"].strip()) if git_status["status"] == "passed" else None
    required = ["rustfmt", "clippy", "rust_tests", "repository_contract", "simulations", "paper"]
    verified = not dirty and all(results[name]["status"] == "passed" for name in required)
    receipt = {
        "schema_version": "1.0.0",
        "artifact_kind": "release_verification_receipt",
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "source_commit": source_commit,
        "source_tree_dirty": dirty,
        "release_verified": verified,
        "toolchains": {
            "rustc": version(["rustc", "--version"]),
            "cargo": version(["cargo", "--version"]),
            "python": version([".venv/bin/python", "--version"]),
            "latexmk": version(["latexmk", "--version"]),
        },
        "commands": results,
        "artifacts": {
            "paper_pdf_sha256": sha256(ROOT / "paper" / "main.pdf"),
            "site_pdf_sha256": sha256(ROOT / "site" / "mechanism_interferometry.pdf"),
            "wasm_sha256": sha256(ROOT / "site" / "pkg" / "mic_bg.wasm"),
        },
        "browser_assertions": {
            "status": "not_run",
            "count": 0,
            "note": "No browser assertion command is currently part of the required repository gate.",
        },
        "scope_note": "Passing commands verify this exact source snapshot only. Adapter-feature and physical-device claims require their own recorded runs.",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "release_verified": verified}))
    return 0 if verified else 1


if __name__ == "__main__":
    raise SystemExit(main())
