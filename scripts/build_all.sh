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
printf '\033[1;36m[1/6] Regenerating example datasets\033[0m\n'
"${PYTHON[@]}" "$ROOT/scripts/generate_example_data.py" >/dev/null
printf '\033[1;36m[2/6] Regenerating exact simulations and figures\033[0m\n'
"${PYTHON[@]}" "$ROOT/scripts/generate_simulations.py" >/dev/null
printf '\033[1;36m[3/6] Building paper\033[0m\n'
(cd "$ROOT/paper" && latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex >/dev/null)
cp "$ROOT/paper/main.pdf" "$ROOT/site/mechanism_interferometry.pdf"
printf '\033[1;36m[4/6] Generating repository manifest\033[0m\n'
"${PYTHON[@]}" "$ROOT/scripts/generate_repository_manifest.py" >/dev/null
printf '\033[1;36m[5/6] Checking repository contracts\033[0m\n'
"${PYTHON[@]}" "$ROOT/scripts/check_repo.py"
printf '\033[1;36m[6/6] Complete\033[0m\n'
