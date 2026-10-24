#!/usr/bin/env bash
# Antigravity-native duplicate of hooks/claude/scan-secrets.sh.
# Scans content being written for accidental secrets.
# PreToolUse hook for write_to_file | replace_file_content | multi_replace_file_content.
# Always exits 0 -- the decision lives in stdout, not the exit code.
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

# Requires jq for JSON parsing. Fail open if missing (not a file-protection hook).
command -v jq >/dev/null 2>&1 || allow

INPUT=$(cat)
TOOL=$(printf '%s' "$INPUT" | jq -r '.toolCall.name // empty')

case "$TOOL" in
  write_to_file)
    CONTENT=$(printf '%s' "$INPUT" | jq -r '.toolCall.args.CodeContent // empty')
    ;;
  replace_file_content)
    CONTENT=$(printf '%s' "$INPUT" | jq -r '.toolCall.args.ReplacementContent // empty')
    ;;
  multi_replace_file_content)
    CONTENT=$(printf '%s' "$INPUT" | jq -r '[.toolCall.args.ReplacementChunks[]?.ReplacementContent] | join("\n")')
    ;;
  *)
    allow
    ;;
esac

[ -z "$CONTENT" ] && allow

# --- High-confidence secret patterns ---

MATCHES=""

# AWS Access Key IDs
if echo "$CONTENT" | grep -qE 'AKIA[0-9A-Z]{16}'; then
  MATCHES="$MATCHES AWS access key (AKIA...);"
fi

# AWS Secret Access Keys (40 chars base64 after a key assignment)
if echo "$CONTENT" | grep -qiE '(aws_secret_access_key|secret_key)[[:space:]]*[=:][[:space:]]*["'\''"]?[A-Za-z0-9/+=]{40}'; then
  MATCHES="$MATCHES AWS secret key;"
fi

# GitHub tokens (PAT, OAuth, App)
if echo "$CONTENT" | grep -qE '(ghp_|gho_|ghs_|ghr_|github_pat_)[a-zA-Z0-9_]{20,}'; then
  MATCHES="$MATCHES GitHub token;"
fi

# OpenAI / Stripe / Anthropic style keys (sk-..., incl. hyphenated sk-ant-api03-...)
if echo "$CONTENT" | grep -qE 'sk-[a-zA-Z0-9-]{20,}'; then
  MATCHES="$MATCHES API key (sk-...);"
fi

# Slack tokens
if echo "$CONTENT" | grep -qE 'xox[bpras]-[0-9a-zA-Z-]{10,}'; then
  MATCHES="$MATCHES Slack token;"
fi

# Private key blocks
if echo "$CONTENT" | grep -qE -- '-----BEGIN[[:space:]]+(RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----'; then
  MATCHES="$MATCHES private key block;"
fi

# Connection strings with embedded credentials
if echo "$CONTENT" | grep -qE '(mongodb|postgres|mysql|redis|amqp|smtp)(\+[a-z]+)?://[^:[:space:]]+:[^@[:space:]]+@'; then
  MATCHES="$MATCHES connection string with credentials;"
fi

# Generic password/secret/token assignments with literal string values
# Matches: password = "actual_value", SECRET_KEY: 'actual_value', api_token="actual_value"
# Excludes: env var references like process.env.*, os.environ.*, ${...}, getenv(...)
if echo "$CONTENT" | grep -qiE '(password|secret|token|api_key|apikey|api_secret)[[:space:]]*[=:][[:space:]]*["'\''"][^"'\''"]{8,}["'\''"]' &&
  ! echo "$CONTENT" | grep -qiE '(password|secret|token|api_key|apikey|api_secret)[[:space:]]*[=:][[:space:]]*["'\''"]?(process\.env|os\.environ|getenv|\$\{|ENV\[|env\()'; then
  MATCHES="$MATCHES hardcoded credential;"
fi

if [ -n "$MATCHES" ]; then
  # Use "ask" not "deny". Warn the user but let them override (could be test fixtures)
  gate ask "Possible secret detected in content:$MATCHES Review carefully before allowing."
fi

allow
