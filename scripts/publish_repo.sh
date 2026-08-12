#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPOSITORY="${1:-Dicklesworthstone/mechanism_interferometry_causality}"
VISIBILITY="${2:-public}"
DESCRIPTION="Mechanism Interferometry: a gauge-invariant soft-intervention certificate for causal modularity"

case "$VISIBILITY" in
  public|private) ;;
  *)
    printf 'error: visibility must be public or private, got %s\n' "$VISIBILITY" >&2
    exit 2
    ;;
esac

EXPECTED_VISIBILITY="$(printf '%s' "$VISIBILITY" | tr '[:lower:]' '[:upper:]')"

red='\033[1;31m'
green='\033[1;32m'
cyan='\033[1;36m'
reset='\033[0m'

fail() {
  printf '%berror:%b %s\n' "$red" "$reset" "$*" >&2
  exit 2
}

command -v git >/dev/null 2>&1 || fail "git is required"
command -v gh >/dev/null 2>&1 || fail "GitHub CLI is required; install gh and authenticate it before publishing"
gh auth status >/dev/null 2>&1 || fail "GitHub CLI is not authenticated; run gh auth login"

git -C "$ROOT" rev-parse --verify HEAD >/dev/null 2>&1 || fail "$ROOT is not a committed git repository"
[[ -z "$(git -C "$ROOT" status --porcelain)" ]] || fail "working tree is not clean; commit or discard changes before publishing"

branch="$(git -C "$ROOT" branch --show-current)"
[[ "$branch" == "main" ]] || fail "expected branch main, found ${branch:-detached HEAD}"

printf '%bPublishing %s from commit %s%b\n' "$cyan" "$REPOSITORY" "$(git -C "$ROOT" rev-parse --short=12 HEAD)" "$reset"

if gh repo view "$REPOSITORY" >/dev/null 2>&1; then
  visibility="$(gh repo view "$REPOSITORY" --json visibility --jq '.visibility')"
  [[ "$visibility" == "$EXPECTED_VISIBILITY" ]] || fail "$REPOSITORY already exists with visibility $visibility, expected $EXPECTED_VISIBILITY"
  remote_url="$(gh repo view "$REPOSITORY" --json sshUrl --jq '.sshUrl')"
  if git -C "$ROOT" remote get-url origin >/dev/null 2>&1; then
    current_url="$(git -C "$ROOT" remote get-url origin)"
    [[ "$current_url" == "$remote_url" ]] || git -C "$ROOT" remote set-url origin "$remote_url"
  else
    git -C "$ROOT" remote add origin "$remote_url"
  fi
  git -C "$ROOT" push --set-upstream origin main
else
  gh repo create "$REPOSITORY" \
    "--$VISIBILITY" \
    --description "$DESCRIPTION" \
    --source "$ROOT" \
    --remote origin \
    --push
fi

actual_visibility="$(gh repo view "$REPOSITORY" --json visibility --jq '.visibility')"
[[ "$actual_visibility" == "$EXPECTED_VISIBILITY" ]] || fail "postcondition failed: repository visibility is $actual_visibility, expected $EXPECTED_VISIBILITY"
printf '%bRepository published successfully (%s): %s%b\n' "$green" "$actual_visibility" "$REPOSITORY" "$reset"
