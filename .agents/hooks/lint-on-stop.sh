#!/usr/bin/env bash
# Antigravity-native duplicate of hooks/claude/lint-on-stop.sh.
# Same dirty-via-git / Stop pattern as typecheck-on-stop.sh, for linting.
#
# ponytail: intentionally duplicated logic, not shared with the Claude-side
# script. Keep both in sync by hand when detection rules change.
#
# NOTE: undocumented -- if multiple independent Stop hooks (this one and
# typecheck-on-stop.sh) each request "continue" in the same cycle, how
# Antigravity combines them isn't specified anywhere. Needs live verification.

set -uo pipefail

ok() {
  printf '{"decision":"ok"}\n'
  exit 0
}
cont() {
  jq -cn --arg r "$1" '{decision:"continue", reason:$r}'
  exit 0
}

command -v jq >/dev/null 2>&1 || ok

INPUT=$(cat)
CONVERSATION_ID=$(printf '%s' "$INPUT" | jq -r '.conversationId // "default"')
ROOT=$(printf '%s' "$INPUT" | jq -r '.workspacePaths[0] // empty')
[ -z "$ROOT" ] && ROOT="$PWD"

MARKER_DIR="/tmp/agent-starter-hooks-antigravity"
MARKER="$MARKER_DIR/${CONVERSATION_ID}.lint-continued"

git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1 || ok

if [ -z "$(git -C "$ROOT" status --porcelain 2>/dev/null)" ]; then
  rm -f "$MARKER"
  ok
fi

detect_pm() {
  if [ -f "$ROOT/bun.lockb" ] || [ -f "$ROOT/bun.lock" ]; then
    echo "bun"
  elif [ -f "$ROOT/pnpm-lock.yaml" ]; then
    echo "pnpm"
  elif [ -f "$ROOT/yarn.lock" ]; then
    echo "yarn"
  else
    echo "npm"
  fi
}

OUTPUT=""
EXIT=0
RAN=false

# JS/TS: run the project's own lint script via the right PM.
if [ -f "$ROOT/package.json" ]; then
  if jq -e '.scripts.lint' "$ROOT/package.json" >/dev/null 2>&1; then
    PM=$(detect_pm)
    OUTPUT=$(cd "$ROOT" && "$PM" run lint 2>&1)
    EXIT=$?
    RAN=true
  fi
fi

# Go: golangci-lint if installed. No fallback (go vet already covered by typecheck-on-stop.sh).
if [ "$RAN" = false ] && [ -f "$ROOT/go.mod" ] && command -v golangci-lint >/dev/null 2>&1; then
  OUTPUT=$(cd "$ROOT" && golangci-lint run 2>&1)
  EXIT=$?
  RAN=true
fi

# Rust: clippy if installed.
if [ "$RAN" = false ] && [ -f "$ROOT/Cargo.toml" ] && command -v cargo-clippy >/dev/null 2>&1; then
  OUTPUT=$(cd "$ROOT" && cargo clippy --all-targets --quiet 2>&1)
  EXIT=$?
  RAN=true
fi

if [ "$RAN" = false ] || [ "$EXIT" -eq 0 ]; then
  rm -f "$MARKER"
  ok
fi

if [ -f "$MARKER" ]; then
  ok
fi
mkdir -p "$MARKER_DIR"
touch "$MARKER"
cont "lint-on-stop: lint errors found
$OUTPUT"
