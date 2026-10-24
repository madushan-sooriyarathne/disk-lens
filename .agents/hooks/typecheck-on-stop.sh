#!/usr/bin/env bash
# Antigravity-native duplicate of hooks/claude/typecheck-on-stop.sh.
# Runs the project's type-check once the execution loop is about to fully
# stop, not per-file. Antigravity's PostToolUse carries no tool args, so
# there's no per-edit marker to set -- dirtiness is read live from git at
# Stop time instead.
#
# Stop input: {executionNum, terminationReason, error, fullyIdle,
# conversationId, workspacePaths, ...}. Output requires "decision"; "reason"
# only reaches the agent when decision is "continue" (re-enters the loop).
#
# ponytail: intentionally duplicated logic, not shared with the Claude-side
# script. Keep both in sync by hand when detection rules change.
#
# NOTE: undocumented -- if multiple independent Stop hooks (this one and
# lint-on-stop.sh) each request "continue" in the same cycle, how Antigravity
# combines them isn't specified anywhere. Needs live verification.

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
MARKER="$MARKER_DIR/${CONVERSATION_ID}.typecheck-continued"

git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1 || ok

# Nothing changed -- clear any stale "already nagged" marker and stop.
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

# JS/TS: run the project's own typecheck/check-types script via the right PM.
if [ -f "$ROOT/package.json" ]; then
  SCRIPT=""
  if jq -e '.scripts.typecheck' "$ROOT/package.json" >/dev/null 2>&1; then
    SCRIPT="typecheck"
  elif jq -e '.scripts["check-types"]' "$ROOT/package.json" >/dev/null 2>&1; then
    SCRIPT="check-types"
  fi
  if [ -n "$SCRIPT" ]; then
    PM=$(detect_pm)
    OUTPUT=$(cd "$ROOT" && "$PM" run "$SCRIPT" 2>&1)
    EXIT=$?
    RAN=true
  fi
fi

# Go: vet does the type-checking job, no separate typecheck command exists.
if [ "$RAN" = false ] && [ -f "$ROOT/go.mod" ] && command -v go >/dev/null 2>&1; then
  OUTPUT=$(cd "$ROOT" && go vet ./... 2>&1)
  EXIT=$?
  RAN=true
fi

# Rust: cargo check is the type-check, no codegen.
if [ "$RAN" = false ] && [ -f "$ROOT/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
  OUTPUT=$(cd "$ROOT" && cargo check --quiet 2>&1)
  EXIT=$?
  RAN=true
fi

if [ "$RAN" = false ] || [ "$EXIT" -eq 0 ]; then
  rm -f "$MARKER"
  ok
fi

# Errors found. Nag once per conversation; don't loop forever if they persist.
if [ -f "$MARKER" ]; then
  ok
fi
mkdir -p "$MARKER_DIR"
touch "$MARKER"
cont "typecheck-on-stop: type errors found
$OUTPUT"
