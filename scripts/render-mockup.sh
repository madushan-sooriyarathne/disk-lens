#!/bin/bash
# Renders docs/mockup-v2/mockup.html to docs/mockup-v2.png (light) and
# docs/mockup-v2-dark.png (dark) with headless Chrome. Writes only those two files.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
page="file://$repo/docs/mockup-v2/mockup.html"
# Two columns of 1280x820 frames, four rows, plus headings and gaps (see the .board styles).
size="2680,3870"

if [[ ! -x "$chrome" ]]; then
	echo "Google Chrome not found at $chrome (set CHROME)" >&2
	exit 1
fi

render() {
	"$chrome" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
		--allow-file-access-from-files --virtual-time-budget=2000 \
		--window-size="$size" --screenshot="$2" "$1" 2>/dev/null
}

render "$page" "$repo/docs/mockup-v2.png"
render "$page#dark" "$repo/docs/mockup-v2-dark.png"
echo "Wrote docs/mockup-v2.png and docs/mockup-v2-dark.png"
