#!/usr/bin/env bash
# Antigravity-native duplicate of hooks/claude/protect-files.sh.
# Blocks edits to sensitive or generated files.
# PreToolUse hook. Reads Antigravity's {"toolCall":{"name","args"}} stdin;
# always exits 0 -- the decision lives in stdout, not the exit code.
#
# ponytail: intentionally duplicated logic, not shared with the Claude-side
# script (dir-per-host, no translation shim). Keep both in sync by hand when
# detection rules change.

set -uo pipefail

allow() {
  printf '{"decision":"allow"}\n'
  exit 0
}
gate() {
  jq -cn --arg d "$1" --arg r "$2" '{decision:$d, reason:$r}'
  exit 0
}

command -v jq >/dev/null 2>&1 || {
  printf '{"decision":"deny","reason":"jq is required for file protection hooks but is not installed."}\n'
  exit 0
}

INPUT=$(cat)
FILE_PATH=$(printf '%s' "$INPUT" | jq -r '.toolCall.args.TargetFile // empty' 2>/dev/null || true)
[ -z "$FILE_PATH" ] && allow

BASENAME=$(basename -- "$FILE_PATH")
# Case-insensitive comparison copy
BASENAME_LC=$(printf '%s' "$BASENAME" | tr '[:upper:]' '[:lower:]')
PATH_LC=$(printf '%s' "$FILE_PATH" | tr '[:upper:]' '[:lower:]')

# Protected basename patterns. Matched case-insensitively via BASENAME_LC.
PROTECTED_PATTERNS=(
  ".env"
  ".env.*"
  "*.pem"
  "*.key"
  "*.crt"
  "*.p12"
  "*.pfx"
  "id_rsa"
  "id_ed25519"
  "credentials.json"
  ".npmrc"
  ".pypirc"
  "package-lock.json"
  "yarn.lock"
  "pnpm-lock.yaml"
  "*.gen.ts"
  "*.generated.*"
  "*.min.js"
  "*.min.css"
)

shopt -s nocasematch 2>/dev/null || true
for pattern in "${PROTECTED_PATTERNS[@]}"; do
  # Using bash case with nocasematch for case-insensitive glob match.
  case "$BASENAME_LC" in
    $pattern)
      gate deny "Protected file: $BASENAME matches pattern '$pattern'"
      ;;
  esac
done

# Sensitive directories (use lower-cased path for case-insensitive on mac/Windows).
case "$PATH_LC" in
  .git/* | */.git/*)
    gate deny "Cannot edit files inside .git/"
    ;;
  secrets/* | */secrets/*)
    gate deny "Cannot edit files inside secrets/"
    ;;
  .env | .env.* | */.env | */.env.*)
    gate deny "Cannot edit .env files"
    ;;
  .claude/hooks/* | */.claude/hooks/*)
    gate deny "Cannot edit hook scripts. These enforce security boundaries."
    ;;
  .claude/settings.json | */.claude/settings.json | .claude/settings.local.json | */.claude/settings.local.json)
    gate ask "Editing settings.json. This controls permissions and hooks. Confirm this change."
    ;;
esac

allow
