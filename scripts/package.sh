#!/usr/bin/env bash
# Builds the universal (arm64 + x86_64) DiskLens.app and DMG under <repo>/target.
# Writes only inside target/; needs `cargo install cargo-packager --locked` and the
# two rustup targets from rust-toolchain.toml.
#
# Signing: ad-hoc ("-") unless DISKLENS_SIGNING_IDENTITY names a Developer ID
# Application identity. Notarization is done by cargo-packager itself and only when
# it sees APPLE_API_KEY, APPLE_API_ISSUER and APPLE_API_KEY_PATH (or APPLE_ID,
# APPLE_PASSWORD and APPLE_TEAM_ID) in the environment; otherwise it is skipped.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$REPO"

IDENTITY="${DISKLENS_SIGNING_IDENTITY:--}"
MAX_DMG_BYTES=$((25 * 1024 * 1024))
ARM="target/aarch64-apple-darwin/release/disklens"
X86="target/x86_64-apple-darwin/release/disklens"
UNIVERSAL_DIR="target/universal-apple-darwin/release"
APP="target/packager/DiskLens.app"

for triple in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --release --bin disklens --target "$triple"
done

mkdir -p "$UNIVERSAL_DIR"
lipo -create "$ARM" "$X86" -output "$UNIVERSAL_DIR/disklens"
archs="$(lipo -archs "$UNIVERSAL_DIR/disklens")"
for arch in arm64 x86_64; do
  case " $archs " in
    *" $arch "*) ;;
    *) echo "universal binary has archs '$archs', missing $arch" >&2; exit 1 ;;
  esac
done

# The identity cannot come from the environment, so merge it into the Cargo.toml
# metadata and hand the result over as a raw JSON config (relative paths stay repo-relative).
config="$(cargo metadata --no-deps --format-version 1 | jq -c --arg id "$IDENTITY" '
  .packages[] | select(.name == "disklens") as $p
  | $p.metadata.packager * {
      name: $p.name, version: $p.version, description: $p.description,
      authors: $p.authors, macos: {"signing-identity": $id}
    }')"
cargo packager --config "$config"

dmg="$(ls target/packager/*.dmg)"
dmg_bytes="$(stat -f %z "$dmg")"
if [ "$dmg_bytes" -ge "$MAX_DMG_BYTES" ]; then
  echo "$dmg is $dmg_bytes bytes, limit is $MAX_DMG_BYTES" >&2
  exit 1
fi
du -sh "$dmg" "$APP"
