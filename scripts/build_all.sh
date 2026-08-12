#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
printf '\033[1;36m[1/6] Regenerating example datasets\033[0m\n'
python "$ROOT/scripts/generate_example_data.py" >/dev/null
printf '\033[1;36m[2/6] Regenerating exact simulations and figures\033[0m\n'
python "$ROOT/scripts/generate_simulations.py" >/dev/null
printf '\033[1;36m[3/6] Building paper\033[0m\n'
(cd "$ROOT/paper" && latexmk -pdf -interaction=nonstopmode -halt-on-error main.tex >/dev/null)
cp "$ROOT/paper/main.pdf" "$ROOT/site/mechanism_interferometry.pdf"
printf '\033[1;36m[4/6] Generating repository manifest\033[0m\n'
python "$ROOT/scripts/generate_repository_manifest.py" >/dev/null
printf '\033[1;36m[5/6] Checking repository contracts\033[0m\n'
python "$ROOT/scripts/check_repo.py"
printf '\033[1;36m[6/6] Complete\033[0m\n'
