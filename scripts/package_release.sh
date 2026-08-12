#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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
python - "$ROOT" "$OUT_DIR" "$NAME" <<'PY'
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
