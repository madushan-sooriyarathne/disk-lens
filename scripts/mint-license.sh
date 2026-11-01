#!/bin/bash
# Mints a signed license token (docs/PLAN.md section 3) for development and QA and
# prints only the token. The signing key stays where DISKLENS_LICENSE_KEY_PATH points;
# its public half is assets/license.pub. Needs the minisign CLI.
#
#   DISKLENS_LICENSE_KEY_PATH=... scripts/mint-license.sh <pro|trial> [max_major]
#
# max_major defaults to the major version in Cargo.toml. A trial ends in 7 days.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
KIND="${1:-}"
case "$KIND" in
pro | trial) ;;
*)
	echo "usage: scripts/mint-license.sh <pro|trial> [max_major]" >&2
	exit 2
	;;
esac
if [[ -z "${DISKLENS_LICENSE_KEY_PATH:-}" ]]; then
	echo "DISKLENS_LICENSE_KEY_PATH is not set (path to the minisign license secret key)" >&2
	exit 1
fi
command -v minisign >/dev/null || {
	echo "minisign is not installed (brew install minisign)" >&2
	exit 1
}

MAX_MAJOR="${2:-$(sed -n '/^version *=/{s/^version *= *"\([0-9][0-9]*\)\..*/\1/p;q;}' "$REPO/Cargo.toml")}"
[[ "$MAX_MAJOR" =~ ^[0-9]+$ ]] || {
	echo "max_major must be a number" >&2
	exit 2
}

ID="lic_$(head -c 4000 /dev/urandom | LC_ALL=C tr -dc 'a-z2-7' | head -c 26)"
NOW="$(date +%s)"
if [[ "$KIND" == trial ]]; then EXPIRES=$((NOW + 7 * 86400)); else EXPIRES=null; fi

WORK="$(mktemp -d)"
trap 'rm -rf -- "$WORK"' EXIT

# No trailing newline: the signature covers these exact bytes.
printf '{"v":1,"id":"%s","kind":"%s","issued":%s,"max_major":%s,"expires":%s}' \
	"$ID" "$KIND" "$NOW" "$MAX_MAJOR" "$EXPIRES" >"$WORK/payload.json"
minisign -S -s "$DISKLENS_LICENSE_KEY_PATH" -m "$WORK/payload.json" -x "$WORK/payload.minisig" -t "$ID" >&2

b64url() { base64 <"$1" | tr -d '\n' | tr '+/' '-_' | tr -d '='; }
echo "DL1.$(b64url "$WORK/payload.json").$(b64url "$WORK/payload.minisig")"
