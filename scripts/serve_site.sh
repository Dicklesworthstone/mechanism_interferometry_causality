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
# 8765 is MCP Agent Mail's well-known port. A preview server there collides with
# it: `http.server` binds the wildcard address, so it takes *:8765 alongside
# am's 127.0.0.1:8765 and clients reach whichever the resolver hands them first.
PORT="${PORT:-8099}"
# Bind loopback explicitly. The default wildcard bind publishes this directory
# to every interface on the machine, which a local doc preview has no business
# doing.
BIND="${BIND:-127.0.0.1}"
printf '\033[1;36mServing Mechanism Interferometry at http://%s:%s\033[0m\n' "$BIND" "$PORT"
exec "${PYTHON[@]}" -m http.server "$PORT" --bind "$BIND" --directory "$ROOT/site"
