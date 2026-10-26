#!/bin/bash
# Regenerates assets/icons/icon.icns from assets/icons/icon.svg. Writes only
# assets/icons/icon.icns and target/icon/.
#
# Uses only tools that ship with macOS: qlmanage renders the SVG (WebKit),
# sips scales it, iconutil packs the iconset.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
SVG="$REPO/assets/icons/icon.svg"
WORK="$REPO/target/icon"
ICONSET="$WORK/icon.iconset"

rm -rf -- "$WORK"
mkdir -p "$ICONSET"

qlmanage -t -s 1024 -o "$WORK" "$SVG" >/dev/null
MASTER="$WORK/icon.svg.png"
[ -f "$MASTER" ] || {
	echo "qlmanage did not render $SVG" >&2
	exit 1
}

for size in 16 32 128 256 512; do
	sips -z "$size" "$size" "$MASTER" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
	double=$((size * 2))
	sips -z "$double" "$double" "$MASTER" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil -c icns "$ICONSET" -o "$REPO/assets/icons/icon.icns"
echo "wrote assets/icons/icon.icns"
