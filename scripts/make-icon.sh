#!/bin/bash
# Regenerates assets/icons/icon-256.png and icon-256-light.png from the brand kit's
# glass icons, icon.icns (dark, also the bundle icon) and icon-light.icns, which
# already hold the small cut in the 16 and 32 px slots and transparent corners.
# The onboarding draws the PNGs, which decode far faster than an SVG: the light
# one in dark mode, the dark one in light mode. Writes only those two files and
# target/icon/.
#
# The kit's renders are used as they are: qlmanage, the only SVG renderer that
# ships with macOS, flattens the glass onto white.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
ICONS="$REPO/assets/icons"
WORK="$REPO/target/icon"

rm -rf -- "$WORK"
mkdir -p "$WORK"

iconutil -c iconset "$ICONS/icon.icns" -o "$WORK/dark.iconset"
iconutil -c iconset "$ICONS/icon-light.icns" -o "$WORK/light.iconset"
cp "$WORK/dark.iconset/icon_128x128@2x.png" "$ICONS/icon-256.png"
cp "$WORK/light.iconset/icon_128x128@2x.png" "$ICONS/icon-256-light.png"
echo "wrote assets/icons/icon-256.png and icon-256-light.png"
