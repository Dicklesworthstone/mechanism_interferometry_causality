#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Resolve an interpreter that actually carries the project's dependencies.
#
# A bare `python` is the wrong choice twice over: modern macOS ships none at all, so
# the command aborts before doing anything, and where it does exist it is usually a
# system interpreter without `jsonschema` or `lxml`, which these checks import. The
# project is uv-managed, so prefer `uv run`, and fall back to `python3` for the paths
# that need no third-party imports.
if command -v uv >/dev/null 2>&1; then
  PYTHON=(uv run --project "$ROOT" python)
elif command -v python3 >/dev/null 2>&1; then
  PYTHON=(python3)
else
  printf 'error: need uv (preferred) or python3 on PATH\n' >&2
  exit 2
fi
OUT_DIR="${1:-$ROOT/dist}"
NAME="mechanism_interferometry_causality"
mkdir -p "$OUT_DIR"

if ! git -C "$ROOT" rev-parse --verify HEAD >/dev/null 2>&1; then
  printf '\033[1;31merror: initialize and commit the repository before packaging\033[0m\n' >&2
  exit 2
fi

printf '\033[1;36mPackaging source archives\033[0m\n'
git -C "$ROOT" archive --format=zip --prefix="$NAME/" -o "$OUT_DIR/$NAME.zip" HEAD
git -C "$ROOT" archive --format=tar.gz --prefix="$NAME/" -o "$OUT_DIR/$NAME.tar.gz" HEAD
git -C "$ROOT" bundle create "$OUT_DIR/$NAME.bundle" --all

printf '\033[1;36mPackaging standalone website\033[0m\n'
"${PYTHON[@]}" - "$ROOT" "$OUT_DIR" "$NAME" <<'PY'
from pathlib import Path
import sys
import zipfile
root, out, name = map(Path, sys.argv[1:])
with zipfile.ZipFile(out / f"{name}_website.zip", "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
    for path in sorted((root / "site").rglob("*")):
        if path.is_file():
            zf.write(path, Path("site") / path.relative_to(root / "site"))
PY

sha256sum \
  "$OUT_DIR/$NAME.zip" \
  "$OUT_DIR/$NAME.tar.gz" \
  "$OUT_DIR/$NAME.bundle" \
  "$OUT_DIR/${NAME}_website.zip" \
  > "$OUT_DIR/SHA256SUMS"
printf '\033[1;32mRelease package written to %s\033[0m\n' "$OUT_DIR"
