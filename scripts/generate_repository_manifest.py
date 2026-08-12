#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "REPOSITORY_MANIFEST.json"
# Must stay a superset of the ephemeral directories in .gitignore. A directory that is
# gitignored but not listed here gets hashed into the manifest and then goes missing in a
# clean checkout, which fails the manifest path-set check for everyone but the author.
IGNORED_DIRECTORIES = {
    ".git", "_renders", "target", "__pycache__", ".venv", "venv", "dist",
    ".wrangler", ".beads", ".ee", ".ntm", ".bv", ".claude",
}
IGNORED_SUFFIXES = {
    ".aux",
    ".bbl",
    ".bcf",
    ".blg",
    ".fdb_latexmk",
    ".fls",
    ".log",
    ".out",
    ".pyc",
    ".run.xml",
    ".toc",
}
IGNORED_FILENAMES = {"REPOSITORY_MANIFEST.json"}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def generated_at() -> str:
    epoch = os.environ.get("SOURCE_DATE_EPOCH")
    if epoch is not None:
        moment = datetime.fromtimestamp(int(epoch), tz=timezone.utc)
    else:
        moment = datetime.now(timezone.utc)
    return moment.isoformat(timespec="seconds").replace("+00:00", "Z")


def included(path: Path) -> bool:
    relative = path.relative_to(ROOT)
    if any(part in IGNORED_DIRECTORIES for part in relative.parts):
        return False
    if path.name in IGNORED_FILENAMES:
        return False
    if path.name.endswith(tuple(IGNORED_SUFFIXES)):
        return False
    return path.is_file()


def build_manifest() -> dict[str, object]:
    files = []
    for path in sorted(ROOT.rglob("*")):
        if not included(path):
            continue
        relative = path.relative_to(ROOT).as_posix()
        files.append({"path": relative, "bytes": path.stat().st_size, "sha256": sha256(path)})
    aggregate = hashlib.sha256()
    for item in files:
        aggregate.update(str(item["path"]).encode())
        aggregate.update(b"\0")
        aggregate.update(str(item["sha256"]).encode())
        aggregate.update(b"\n")
    return {
        "schema_version": "1.0.0",
        "repository": "Dicklesworthstone/mechanism_interferometry_causality",
        "generated_at_utc": generated_at(),
        "hash_algorithm": "sha256",
        "file_count": len(files),
        "aggregate_sha256": aggregate.hexdigest(),
        "files": files,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate the repository content manifest.")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    output = args.output if args.output.is_absolute() else ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(build_manifest(), indent=2) + "\n", encoding="utf-8")
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
