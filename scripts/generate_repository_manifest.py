#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "REPOSITORY_MANIFEST.json"
# Only consulted by the no-git fallback below. Git is the authority when it is available,
# so this list does not need to track .gitignore and must not be relied on to.
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


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


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


def git_index_paths() -> list[Path] | None:
    """Every path in the git index, or None when this is not a usable git checkout.

    `--cached` covers tracked files plus anything already staged, which is exactly the
    set a fresh clone of the next commit will contain.
    """
    try:
        result = subprocess.run(
            ["git", "-C", str(ROOT), "ls-files", "-z", "--cached"],
            capture_output=True,
            check=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    names = result.stdout.decode("utf-8").split("\0")
    return [ROOT / name for name in names if name]


def manifest_paths() -> list[Path]:
    """The manifest's authoritative path set, shared by the generator and the checker.

    Sourced from the git index, so an untracked or gitignored file cannot enter the
    inventory no matter what is lying around the working tree. That failure mode is not
    hypothetical: `.wrangler` caches, a stray hypothesis document, and a directory of
    pilot scripts each got hashed in as required content this way, and each broke every
    clone but the author's. Enumerating from git removes the class rather than adding
    another name to an ignore list.

    Falls back to a filesystem walk filtered by `IGNORED_DIRECTORIES` when git is absent,
    so an extracted source tarball still verifies.
    """
    candidates = git_index_paths()
    if candidates is None:
        candidates = sorted(ROOT.rglob("*"))
    return sorted({path for path in candidates if included(path)})


def git_index_bytes(path: Path) -> bytes:
    relative = path.relative_to(ROOT).as_posix()
    result = subprocess.run(
        ["git", "-C", str(ROOT), "show", f":{relative}"],
        capture_output=True,
        check=True,
    )
    return result.stdout


def build_manifest(*, from_index: bool = False) -> dict[str, object]:
    files = []
    for path in manifest_paths():
        relative = path.relative_to(ROOT).as_posix()
        if from_index:
            data = git_index_bytes(path)
            byte_count = len(data)
            digest = sha256_bytes(data)
        else:
            byte_count = path.stat().st_size
            digest = sha256(path)
        files.append({"path": relative, "bytes": byte_count, "sha256": digest})
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
    parser.add_argument(
        "--from-index",
        action="store_true",
        help="hash staged Git blobs instead of possibly dirty working-tree bytes",
    )
    args = parser.parse_args()
    output = args.output if args.output.is_absolute() else ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(build_manifest(from_index=args.from_index), indent=2) + "\n",
        encoding="utf-8",
    )
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
