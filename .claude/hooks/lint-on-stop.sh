#!/bin/bash
# Runs the project's linter once Claude finishes its turn, not per-file.
# PostToolUse(Edit|Write): marks the session dirty.
# Stop: if dirty, runs the linter for the detected project, then clears the
# marker. Silent on success. On failure, exits 2 so Claude sees the errors
# and keeps working instead of stopping.

if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

INPUT=$(cat)
EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "default"')

MARKER_DIR="/tmp/agent-starter-hooks"
MARKER="$MARKER_DIR/${SESSION_ID}.lint-dirty"

if [ "$EVENT" = "PostToolUse" ]; then
  FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
  [ -z "$FILE_PATH" ] && exit 0

  EXTENSION="${FILE_PATH##*.}"
  case "$EXTENSION" in
    md | txt | svg | png | jpg | jpeg | gif | ico | css | scss | less | html | lock | sh) exit 0 ;;
  esac
  case "$FILE_PATH" in
    */.claude/* | */node_modules/* | */dist/* | */build/* | */.git/* | */target/*) exit 0 ;;
  esac

  mkdir -p "$MARKER_DIR"
  touch "$MARKER"
  exit 0
fi

if [ "$EVENT" != "Stop" ]; then
  exit 0
fi

# Avoid retriggering ourselves if this hook already blocked the stop once.
STOP_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active // false')
if [ "$STOP_ACTIVE" = "true" ]; then
  exit 0
fi

[ -f "$MARKER" ] || exit 0
rm -f "$MARKER"

find_project_root() {
  local dir="$PWD"
  while [ "$dir" != "/" ]; do
    if [ -f "$dir/package.json" ] || [ -f "$dir/go.mod" ] || [ -f "$dir/Cargo.toml" ] || [ -d "$dir/.git" ]; then
      echo "$dir"
      return
    fi
    dir=$(dirname "$dir")
  done
  echo "$PWD"
}

ROOT=$(find_project_root)

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

if [ "$RAN" = true ] && [ "$EXIT" -ne 0 ]; then
  echo "lint-on-stop: lint errors found" >&2
  echo "$OUTPUT" >&2
  exit 2
fi

exit 0
