#!/usr/bin/env bash
# Antigravity-native duplicate of hooks/claude/auto-test.sh.
# Antigravity's PostToolUse carries no tool args, so there's no per-edit file
# path to react to. Instead this runs once per Stop against every file the
# git working tree shows as changed (diff vs HEAD plus untracked new files) --
# same test-matching logic as the Claude-side script, batched instead of
# per-edit. Never requests "continue" -- matches the Claude-side script,
# which reports failures but never blocks.
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

# Search for a matching test file in the usual conventions.
find_test_file() {
  local dir="$1" stem="$2" ext="$3"
  local patterns=(
    "${stem}.test.${ext}"
    "${stem}.spec.${ext}"
    "${stem}_test.${ext}"
    "${stem}_spec.${ext}"
    "test_${stem}.${ext}"
  )

  # Same directory first.
  for pattern in "${patterns[@]}"; do
    [ -f "${dir}/${pattern}" ] && {
      echo "${dir}/${pattern}"
      return
    }
  done

  # __tests__ subdirectory (Jest convention).
  for pattern in "${patterns[@]}"; do
    [ -f "${dir}/__tests__/${pattern}" ] && {
      echo "${dir}/__tests__/${pattern}"
      return
    }
  done

  # Parallel test directory structure (src/foo.ts -> tests/foo.test.ts).
  local rel_dir="${dir#$ROOT/}"
  local test_rel_dir
  for test_root in "tests" "test" "__tests__" "spec"; do
    test_rel_dir=$(echo "$rel_dir" | sed "s|^src/|${test_root}/|;s|^lib/|${test_root}/|")
    for pattern in "${patterns[@]}"; do
      [ -f "${ROOT}/${test_rel_dir}/${pattern}" ] && {
        echo "${ROOT}/${test_rel_dir}/${pattern}"
        return
      }
    done
  done

  # Broad search as last resort, depth-limited to stay fast.
  local found
  for pattern in "${patterns[@]}"; do
    found=$(find "$ROOT" -maxdepth 5 -name "$pattern" -not -path "*/node_modules/*" -not -path "*/.git/*" -print -quit 2>/dev/null)
    [ -n "$found" ] && {
      echo "$found"
      return
    }
  done
}

while IFS= read -r REL; do
  [ -z "$REL" ] && continue
  FILE_PATH="$ROOT/$REL"
  [ -f "$FILE_PATH" ] || continue

  BASENAME=$(basename "$FILE_PATH")
  EXTENSION="${BASENAME##*.}"
  NAME="${BASENAME%.*}"
  DIR=$(dirname "$FILE_PATH")

  # Skip if the edited file IS a test file.
  case "$BASENAME" in
    *.test.* | *.spec.* | *_test.* | *_spec.* | test_* | spec_*) continue ;;
  esac

  # Skip config, style, and non-code files.
  case "$EXTENSION" in
    json | yaml | yml | toml | ini | cfg | env | md | txt | css | scss | less | svg | png | jpg | ico | html) continue ;;
  esac

  # Skip files in non-testable directories.
  case "$FILE_PATH" in
    */.agents/* | */public/* | */static/* | */assets/* | */__mocks__/*) continue ;;
  esac

  TEST_FILE=$(find_test_file "$DIR" "$NAME" "$EXTENSION")
  [ -z "$TEST_FILE" ] && continue

  REL_TEST="${TEST_FILE#$ROOT/}"

  OUTPUT=""
  EXIT=0
  case "$EXTENSION" in
    js | jsx | ts | tsx | mjs | cjs)
      if [ -f "$ROOT/node_modules/.bin/vitest" ]; then
        OUTPUT=$(cd "$ROOT" && npx vitest run "$REL_TEST" 2>&1)
        EXIT=$?
      elif [ -f "$ROOT/node_modules/.bin/jest" ]; then
        OUTPUT=$(cd "$ROOT" && npx jest "$REL_TEST" 2>&1)
        EXIT=$?
      elif [ -f "$ROOT/node_modules/.bin/mocha" ]; then
        OUTPUT=$(cd "$ROOT" && npx mocha "$REL_TEST" 2>&1)
        EXIT=$?
      else
        OUTPUT=$(cd "$ROOT" && npm test -- "$REL_TEST" 2>&1)
        EXIT=$?
      fi
      ;;
    py)
      if command -v pytest >/dev/null 2>&1; then
        OUTPUT=$(cd "$ROOT" && pytest "$REL_TEST" 2>&1)
        EXIT=$?
      elif command -v python3 >/dev/null 2>&1; then
        OUTPUT=$(cd "$ROOT" && python3 -m unittest "$REL_TEST" 2>&1)
        EXIT=$?
      elif command -v python >/dev/null 2>&1; then
        OUTPUT=$(cd "$ROOT" && python -m unittest "$REL_TEST" 2>&1)
        EXIT=$?
      fi
      ;;
    go)
      OUTPUT=$(cd "$DIR" && go test ./... 2>&1)
      EXIT=$?
      ;;
    rs)
      OUTPUT=$(cd "$ROOT" && cargo test 2>&1)
      EXIT=$?
      ;;
    lua)
      if command -v busted >/dev/null 2>&1; then
        OUTPUT=$(cd "$ROOT" && busted "$REL_TEST" 2>&1)
        EXIT=$?
      else
        OUTPUT=$(cd "$ROOT" && lua "$REL_TEST" 2>&1)
        EXIT=$?
      fi
      ;;
    *)
      continue
      ;;
  esac

  if [ "$EXIT" -ne 0 ]; then
    echo "auto-test: failures in $REL_TEST"
    echo "$OUTPUT"
  fi
done <<<"$CHANGED"

ok
