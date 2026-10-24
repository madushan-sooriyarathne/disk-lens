#!/usr/bin/env bash
# Antigravity-native duplicate of hooks/claude/block-dangerous-commands.sh.
# Blocks dangerous shell commands: push to protected branches, force push,
# destructive operations. PreToolUse hook for the `run_command` tool.
# Always exits 0 -- the decision lives in stdout, not the exit code.
#
# Configurable via env:
#   CLAUDE_PROTECTED_BRANCHES  comma list (default: derived from git + main,master)
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
  printf '{"decision":"deny","reason":"jq is required for command protection hooks but is not installed."}\n'
  exit 0
}

INPUT=$(cat)
COMMAND=$(printf '%s' "$INPUT" | jq -r '.toolCall.args.CommandLine // empty' 2>/dev/null || true)
[ -z "$COMMAND" ] && allow

# ── Protected branch list ────────────────────────────────────────────────
DEFAULT_BRANCHES="main,master"
if GIT_DEFAULT=$(git config --get init.defaultBranch 2>/dev/null) && [ -n "$GIT_DEFAULT" ]; then
  DEFAULT_BRANCHES="$DEFAULT_BRANCHES,$GIT_DEFAULT"
fi
PROTECTED_BRANCHES="${CLAUDE_PROTECTED_BRANCHES:-$DEFAULT_BRANCHES}"
# Build a regex alternation: main|master|develop|...
BR_REGEX=$(printf '%s' "$PROTECTED_BRANCHES" | tr ',' '\n' | awk 'NF{printf "%s%s",sep,$0; sep="|"}')

contains_cmd() { printf '%s' "$COMMAND" | grep -qE "$1"; }
contains_icmd() { printf '%s' "$COMMAND" | grep -qiE "$1"; }

# ── Git push protections ────────────────────────────────────────────────
if contains_cmd '(^|[;&|()]+[[:space:]]*)git[[:space:]]+push'; then
  # Explicit refspec to a protected branch (origin main, :main, HEAD:main, remote branch)
  if contains_cmd "git[[:space:]]+push[[:space:]]+[^[:space:]]+[[:space:]]+([^[:space:]]*:)?($BR_REGEX)(\$|[[:space:]])"; then
    MATCHED_BRANCH=$(printf '%s' "$COMMAND" | grep -oE "($BR_REGEX)(\$|[[:space:]])" | head -1 | tr -d '[:space:]')
    gate deny "Blocked: push to protected branch '${MATCHED_BRANCH:-main}'. Use a feature branch and open a PR."
  fi
  if contains_cmd "git[[:space:]]+push.*:($BR_REGEX)(\$|[[:space:]])"; then
    MATCHED_BRANCH=$(printf '%s' "$COMMAND" | grep -oE ":($BR_REGEX)(\$|[[:space:]])" | head -1 | tr -d ': [:space:]')
    gate deny "Blocked: push to protected branch '${MATCHED_BRANCH:-main}' via refspec. Use a feature branch and open a PR."
  fi
  # Bare `git push` while on protected branch
  if contains_cmd 'git[[:space:]]+push[[:space:]]*($|[;&|])'; then
    CURRENT=$(git branch --show-current 2>/dev/null || true)
    if [ -n "$CURRENT" ] && printf '%s' ",$PROTECTED_BRANCHES," | grep -q ",$CURRENT,"; then
      gate deny "Blocked: you are on '$CURRENT' (a protected branch). Switch to a feature branch."
    fi
  fi
  # Force push (but allow --force-with-lease)
  if contains_cmd 'git[[:space:]]+push([[:space:]]+[^[:space:]]+)*[[:space:]]+(-[a-zA-Z]*f[a-zA-Z]*|--force)([[:space:]=]|$)' &&
    ! contains_cmd '\-\-force-with-lease'; then
    gate deny "Blocked: force push is not allowed. Use --force-with-lease if you must overwrite remote."
  fi
fi

# ── Destructive filesystem operations ───────────────────────────────────
# rm -rf targeting root, home, $HOME, $VAR (any unresolved expansion), or parent traversal.
# We normalise quotes before matching so "my folder", '$HOME/trash', etc. Are all inspected.
CMD_NOQUOTE=$(printf '%s' "$COMMAND" | tr -d "'\"")
if printf '%s' "$CMD_NOQUOTE" | grep -qE 'rm[[:space:]]+(-[a-zA-Z]*[[:space:]]+)*-?[a-zA-Z]*r[a-zA-Z]*f[a-zA-Z]*[[:space:]]+(/([[:space:]]|\*|$)|~|\$HOME|\$[A-Za-z_][A-Za-z0-9_]*|\.\./\.\.)'; then
  gate deny "Blocked: recursive force-delete on /, ~, \$HOME, an unresolved \$VAR, or .../.. Path. Specify a concrete safe target."
fi
# rm -rf /usr, /etc, /var, /bin, etc.
if printf '%s' "$CMD_NOQUOTE" | grep -qE 'rm[[:space:]]+(-[a-zA-Z]+[[:space:]]+)*-?[a-zA-Z]*r[a-zA-Z]*f[a-zA-Z]*[[:space:]]+/(usr|etc|var|bin|sbin|lib|opt|root|boot)([[:space:]/]|$)'; then
  gate deny "Blocked: recursive delete targeting a system directory."
fi

# ── Dangerous database operations ───────────────────────────────────────
# DROP TABLE|DATABASE|SCHEMA
if contains_icmd 'DROP[[:space:]]+(TABLE|DATABASE|SCHEMA)[[:space:]]+'; then
  gate deny "Blocked: DROP TABLE/DATABASE/SCHEMA detected. Run manually if intended."
fi
# DELETE FROM without a WHERE on the SAME statement.
# Split on ';' so multi-statement inputs are analysed per-statement.
if printf '%s\n' "$COMMAND" | awk '
  BEGIN { IGNORECASE=1; RS=";" }
  /DELETE[[:space:]]+FROM[[:space:]]+[A-Za-z_][A-Za-z0-9_.]*/ {
    if ($0 !~ /WHERE/) { print "BAD"; exit }
  }
' | grep -q BAD; then
  gate deny "Blocked: DELETE FROM without a WHERE clause. Add a WHERE or run manually."
fi
if contains_icmd 'TRUNCATE[[:space:]]+TABLE'; then
  gate deny "Blocked: TRUNCATE TABLE detected. Run manually if intended."
fi

# ── Dangerous system commands ───────────────────────────────────────────
# chmod: any world-writable/universal mode (0?777 or a+rwx)
if contains_cmd 'chmod([[:space:]]+-[a-zA-Z]+)*[[:space:]]+0?777([[:space:]]|$)' ||
  contains_cmd 'chmod([[:space:]]+-[a-zA-Z]+)*[[:space:]]+a\+rwx([[:space:]]|$)'; then
  gate deny "Blocked: chmod 777 / a+rwx grants everyone full access. Use restrictive perms."
fi

# curl/wget piped to a shell
if contains_cmd '(curl|wget)[[:space:]].*\|[[:space:]]*(sudo[[:space:]]+)?(bash|sh|zsh|ksh|fish|dash|csh)([[:space:]]|$)'; then
  gate deny "Blocked: piping downloaded content directly to a shell is dangerous."
fi

# Disk / partition. Note: only REDIRECTIONS to /dev/ are destructive. `2>/dev/null` is not.
# Pattern matches: `>[ ]*/dev/<something>` but NOT `2>/dev/null` or `&>/dev/null` style for fd-null.
# Strategy: match `>` optionally with whitespace, followed by /dev/<name>, EXCLUDING /dev/null and /dev/stderr/stdout.
if printf '%s' "$COMMAND" | grep -qE '(^|[^0-9&])>[[:space:]]*/dev/[a-zA-Z][a-zA-Z0-9]*' &&
  ! printf '%s' "$COMMAND" | grep -qE '>[[:space:]]*/dev/(null|stdout|stderr|tty|zero|random|urandom)([[:space:]]|$)'; then
  gate deny "Blocked: redirection into a raw device file can destroy data."
fi
if contains_cmd '(^|[;&|[:space:]])(mkfs|mkfs\.[a-z0-9]+)([[:space:]]|$)' ||
  contains_cmd '(^|[;&|[:space:]])dd[[:space:]]+[^|]*(if|of)=/dev/[a-zA-Z]'; then
  gate deny "Blocked: mkfs/dd against a device node. Irreversible data loss."
fi

# ── Destructive git ─────────────────────────────────────────────────────
if contains_cmd 'git[[:space:]]+reset[[:space:]]+--hard'; then
  gate deny "Blocked: git reset --hard discards uncommitted changes permanently."
fi
if contains_cmd 'git[[:space:]]+clean[[:space:]]+-[a-zA-Z]*f'; then
  gate deny "Blocked: git clean -f permanently deletes untracked files."
fi

# ── Accidental package publishing ───────────────────────────────────────
# Allow --dry-run variants (npm publish --dry-run is safe and common in CI).
publish_patterns=(
  '(npm|yarn|pnpm|bun)[[:space:]]+publish'
  'cargo[[:space:]]+publish'
  'gem[[:space:]]+push'
  'twine[[:space:]]+upload'
)
for pat in "${publish_patterns[@]}"; do
  if contains_cmd "$pat" && ! contains_cmd '(^|[[:space:]])(--dry-run|-n)([[:space:]=]|$)'; then
    gate deny "Blocked: publishing packages should run in CI or manually, not via Claude."
  fi
done

allow
