#!/bin/bash
# Builds a synthetic tree of 2,000,000 empty files at <repo>/.sandbox/perf for the
# Phase 5 performance pass (PRD section 10: peak memory under 400 MB for 2M files).
# 20 top folders x 100 subfolders x 100 leaf folders x 10 files: 202,121
# folders, about the files-per-folder ratio of a real home folder (the scanner's
# memory grows with folders, not files). Empty files take inodes, not blocks,
# so the tree costs almost no disk space. Idempotent: wipes and rebuilds only
# that directory. scripts/make-fixture.sh also removes it (it wipes .sandbox).
set -euo pipefail

TOP=20
SUB=100
LEAF=100
FILES=10

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
PERF="$REPO/.sandbox/perf"

# Safety: the only directory this script may delete.
case "$PERF" in
"$REPO"/.sandbox/perf) ;;
*)
	echo "refusing: perf path escapes repo: $PERF" >&2
	exit 1
	;;
esac
if [ -L "$REPO/.sandbox" ] || [ -L "$PERF" ]; then
	echo "refusing: symlink in $PERF" >&2
	exit 1
fi

rm -rf -- "$PERF"
mkdir -p "$PERF"
leaves=$(seq -f "d%g" 1 "$LEAF")
files=$(for d in $leaves; do seq -f "$d/f%g" 1 "$FILES"; done)
for t in $(seq 1 "$TOP"); do
	for s in $(seq 1 "$SUB"); do
		dir="$PERF/top$t/sub$s"
		mkdir -p "$dir"
		# shellcheck disable=SC2086 # word splitting is the point: one call per subfolder.
		(cd "$dir" && mkdir $leaves && touch $files)
	done
done
echo "built $((TOP * SUB * LEAF * FILES)) files in $PERF"
