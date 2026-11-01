#!/bin/bash
# Regenerates assets/icons/icon.icns, icon-256.png and icon-256-light.png from the
# brand kit's glass icon: icon.svg (dark master), icon-small.svg (the 16 and 32 px
# cut) and icon-light.svg. The onboarding draws the PNGs, which decode far faster
# than the SVGs, and picks the light one in light mode. Writes only those three
# files and target/icon/.
#
# Uses only tools that ship with macOS: qlmanage renders the SVGs (WebKit),
# sips scales them, iconutil packs the iconset.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
ICONS="$REPO/assets/icons"
WORK="$REPO/target/icon"
ICONSET="$WORK/icon.iconset"

rm -rf -- "$WORK"
mkdir -p "$ICONSET"

render() {
	qlmanage -t -s "$2" -o "$WORK" "$ICONS/$1" >/dev/null
	[ -f "$WORK/$1.png" ] || {
		echo "qlmanage did not render $1" >&2
		exit 1
	}
}
render icon.svg 1024
render icon-small.svg 1024
render icon-light.svg 256

# Slots of 32 px and less use the small cut, as the brand kit specifies.
slot() {
	local px=$1 name=$2 master="$WORK/icon.svg.png"
	[ "$px" -le 32 ] && master="$WORK/icon-small.svg.png"
	sips -z "$px" "$px" "$master" --out "$ICONSET/$name.png" >/dev/null
}
for size in 16 32 128 256 512; do
	slot "$size" "icon_${size}x${size}"
	slot $((size * 2)) "icon_${size}x${size}@2x"
done

iconutil -c icns "$ICONSET" -o "$ICONS/icon.icns"
cp "$ICONSET/icon_128x128@2x.png" "$ICONS/icon-256.png"
cp "$WORK/icon-light.svg.png" "$ICONS/icon-256-light.png"
echo "wrote assets/icons/icon.icns, icon-256.png and icon-256-light.png"
