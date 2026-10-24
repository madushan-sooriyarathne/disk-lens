#!/usr/bin/env bash
# Antigravity-native duplicate of hooks/claude/format-on-save.sh.
# Antigravity's PostToolUse carries no tool args, so there's no per-edit file
# path to format immediately after a write. Instead this runs once per Stop,
# formatting every file the git working tree shows as changed (diff vs HEAD
# plus untracked new files) -- same formatter cascade as the Claude-side
# script, just batched instead of per-edit. Never requests "continue".
#
# ponytail: intentionally duplicated logic, not shared with the Claude-side
# script. Keep both in sync by hand when detection rules change.

set -uo pipefail

ok() {
  printf '{"decision":"ok"}\n'
  exit 0
}

command -v jq >/dev/null 2>&1 || ok

INPUT=$(cat)
ROOT=$(printf '%s' "$INPUT" | jq -r '.workspacePaths[0] // empty')
[ -z "$ROOT" ] && ROOT="$PWD"

git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1 || ok

CHANGED=$(
  {
    git -C "$ROOT" diff --name-only HEAD -- 2>/dev/null
    git -C "$ROOT" ls-files --others --exclude-standard 2>/dev/null
  } | sort -u
)
[ -z "$CHANGED" ] && ok

while IFS= read -r REL; do
  [ -z "$REL" ] && continue
  FILE_PATH="$ROOT/$REL"
  [ -f "$FILE_PATH" ] || continue

  EXTENSION="${FILE_PATH##*.}"
  FORMATTED=false

  # Biome (JS, TS, JSON, CSS all-in-one). Faster than Prettier; check first.
  if [ "$FORMATTED" = false ] && [ -f "$ROOT/node_modules/.bin/biome" ] && { [ -f "$ROOT/biome.json" ] || [ -f "$ROOT/biome.jsonc" ]; }; then
    case "$EXTENSION" in
      js | jsx | ts | tsx | json | css)
        npx biome format --write "$FILE_PATH" >/dev/null 2>&1 && FORMATTED=true
        ;;
    esac
  fi

  # Prettier (Node.js, TypeScript, web).
  if [ "$FORMATTED" = false ] && [ -f "$ROOT/node_modules/.bin/prettier" ]; then
    HAS_PRETTIER_CONFIG=false
    for cfg in .prettierrc .prettierrc.json .prettierrc.yml .prettierrc.yaml .prettierrc.js .prettierrc.cjs .prettierrc.mjs .prettierrc.toml prettier.config.js prettier.config.cjs prettier.config.mjs; do
      if [ -f "$ROOT/$cfg" ]; then
        HAS_PRETTIER_CONFIG=true
        break
      fi
    done
    if [ "$HAS_PRETTIER_CONFIG" = false ] && [ -f "$ROOT/package.json" ] && grep -q '"prettier"' "$ROOT/package.json" 2>/dev/null; then
      HAS_PRETTIER_CONFIG=true
    fi

    if [ "$HAS_PRETTIER_CONFIG" = true ]; then
      case "$EXTENSION" in
        js | jsx | ts | tsx | json | css | scss | md | yaml | yml | html)
          npx prettier --write "$FILE_PATH" >/dev/null 2>&1 && FORMATTED=true
          ;;
      esac
    fi
  fi

  # Ruff (Python). Modern replacement for Black + isort.
  if [ "$FORMATTED" = false ] && command -v ruff >/dev/null 2>&1; then
    HAS_RUFF_CONFIG=false
    if [ -f "$ROOT/ruff.toml" ] || [ -f "$ROOT/.ruff.toml" ]; then
      HAS_RUFF_CONFIG=true
    elif [ -f "$ROOT/pyproject.toml" ] && grep -q '\[tool\.ruff\]' "$ROOT/pyproject.toml" 2>/dev/null; then
      HAS_RUFF_CONFIG=true
    fi

    if [ "$HAS_RUFF_CONFIG" = true ]; then
      case "$EXTENSION" in
        py)
          ruff format "$FILE_PATH" >/dev/null 2>&1
          ruff check --fix "$FILE_PATH" >/dev/null 2>&1
          FORMATTED=true
          ;;
      esac
    fi
  fi

  # Black + isort (Python). Fallback if Ruff is not configured.
  if [ "$FORMATTED" = false ] && command -v black >/dev/null 2>&1; then
    HAS_BLACK_CONFIG=false
    if [ -f "$ROOT/pyproject.toml" ] && grep -q '\[tool\.black\]' "$ROOT/pyproject.toml" 2>/dev/null; then
      HAS_BLACK_CONFIG=true
    fi

    if [ "$HAS_BLACK_CONFIG" = true ]; then
      case "$EXTENSION" in
        py)
          black --quiet "$FILE_PATH" >/dev/null 2>&1
          command -v isort >/dev/null 2>&1 && isort --quiet "$FILE_PATH" >/dev/null 2>&1
          FORMATTED=true
          ;;
      esac
    fi
  fi

  # Rust (rustfmt is standard, no config check needed).
  if [ "$FORMATTED" = false ] && command -v rustfmt >/dev/null 2>&1; then
    case "$EXTENSION" in
      rs)
        rustfmt "$FILE_PATH" >/dev/null 2>&1 && FORMATTED=true
        ;;
    esac
  fi

  # Go (gofmt is standard, no config check needed).
  if [ "$FORMATTED" = false ] && command -v gofmt >/dev/null 2>&1; then
    case "$EXTENSION" in
      go)
        gofmt -w "$FILE_PATH" >/dev/null 2>&1 && FORMATTED=true
        ;;
    esac
  fi

  # Shell (shfmt is standard; honors .editorconfig, no config check needed).
  if [ "$FORMATTED" = false ] && command -v shfmt >/dev/null 2>&1; then
    case "$EXTENSION" in
      sh | bash)
        shfmt -w "$FILE_PATH" >/dev/null 2>&1 && FORMATTED=true
        ;;
    esac
  fi
done <<<"$CHANGED"

ok
