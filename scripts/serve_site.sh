#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${PORT:-8765}"
printf '\033[1;36mServing Mechanism Interferometry at http://127.0.0.1:%s\033[0m\n' "$PORT"
exec python -m http.server "$PORT" --directory "$ROOT/site"
