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
PORT="${PORT:-8765}"
printf '\033[1;36mServing Mechanism Interferometry at http://127.0.0.1:%s\033[0m\n' "$PORT"
exec "${PYTHON[@]}" -m http.server "$PORT" --directory "$ROOT/site"
