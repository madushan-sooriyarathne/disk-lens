#!/bin/bash
# Builds the fixture sandbox at <repo>/.sandbox/home. Idempotent: wipes and rebuilds
# only that directory. Never touches the real home folder.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
SANDBOX="$REPO/.sandbox"
HOME_DIR="$SANDBOX/home"
OUTSIDE="$SANDBOX/outside"

# Safety: the only directory this script may delete.
case "$SANDBOX" in
  "$REPO"/.sandbox) ;;
  *) echo "refusing: sandbox path escapes repo: $SANDBOX" >&2; exit 1 ;;
esac
if [ -L "$SANDBOX" ]; then echo "refusing: $SANDBOX is a symlink" >&2; exit 1; fi

rm -rf -- "$SANDBOX"
mkdir -p "$HOME_DIR" "$OUTSIDE"
cd "$HOME_DIR"

# file <path> [kb]: create a small real file (default 4 KB), creating parents.
file() { mkdir -p "$(dirname "$1")"; head -c "$(( ${2:-4} * 1024 ))" /dev/zero | tr '\0' 'x' > "$1"; }
G() { git -c user.name=fixture -c user.email=fixture@example.com -c commit.gpgsign=false "$@"; }

# ---- Safe tier targets -------------------------------------------------------
file .npm/_cacache/content-v2/a.bin 64
file .npm/_npx/x/index.js
file .npm/_logs/debug.log
file Library/pnpm/store/v3/files/ab/cd
file .bun/install/cache/pkg/index.js
file Library/Caches/Yarn/v6/pkg.tgz
file .yarn/berry/cache/pkg.zip
file Desktop/web-app/package.json 1
file Desktop/web-app/.next/cache/a.js 32
file Desktop/web-app/.turbo/log.txt
file Desktop/web-app/node_modules/dep/index.js
file .gradle/caches/modules-2/a.jar 64
file .gradle/wrapper/dists/gradle-8/bin
file .gradle/daemon/8.0/daemon.log
file Library/Developer/Xcode/DerivedData/App-abc/Build/out.o 32
file .cache/uv/wheel.whl
file .cache/puppeteer/chrome/chrome.bin
file Library/Caches/ms-playwright/chromium-1/chrome
file Library/Caches/pip/http/a
file Library/Caches/Homebrew/downloads/a.tar.gz
file Library/Caches/CocoaPods/Pods/a
file Library/Caches/go-build/aa/obj
file .cargo/registry/cache/index/crate.crate
file .cargo/registry/src/index/crate/lib.rs
file Library/Caches/com.example.app/cache.db 64
# ---- Review tier targets -----------------------------------------------------
file Library/Android/sdk/platforms/android-34/android.jar 64
file .android/avd/Pixel.avd/userdata.img 64
file .android/cache/a
file .ollama/models/manifests/registry.ollama.ai/library/llama3/8b
file .ollama/models/blobs/sha256-aaaa 64
file .cache/huggingface/hub/models--x/blob 64
file .lmstudio/models/m/model.gguf 64
file Library/Developer/Xcode/Archives/2024-01-01/App.xcarchive/Info.plist
file "Library/Developer/Xcode/iOS DeviceSupport/17.0/Symbols/a"
file Library/Developer/CoreSimulator/Devices/AAAA-BBBB/data/a 16
file Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw 64
file "Library/Application Support/MobileSync/Backup/0000-1111/Info.plist"
file .Trash/old-file.txt 16

# ---- Near misses (must NOT match) -------------------------------------------
file Desktop/not-a-project/.next/a.js 32            # .next without package.json/turbo.json
file Desktop/web-app/node_modules/dep/node_modules/nested/index.js  # nested node_modules
file Library/Caches/com.apple.Safari/a 64           # excluded by app-caches
file Library/Caches/CloudKit/a                      # excluded by app-caches
file Library/Developer/Xcode/DerivedData.bak/a      # near-miss name

# ---- Protected paths ---------------------------------------------------------
file Library/Keychains/login.keychain-db
file .ssh/id_test
file .aws/credentials 1
file Pictures/Photos.photoslibrary/database/a 16

# ---- Links -------------------------------------------------------------------
file "$OUTSIDE/secret.txt" 8
ln -s "$OUTSIDE" Desktop/outside-link               # symlink pointing outside the sandbox home
file Desktop/hardlinks/a.bin 16
ln Desktop/hardlinks/a.bin Desktop/hardlinks/b.bin  # hard link: counted once

# ---- Git repo containing .next, plus worktrees ------------------------------
mkdir -p Desktop/gitrepo
(
  cd Desktop/gitrepo
  file package.json 1
  file .next/cache/a.js 16
  G init -q -b main
  # .next is untracked build output; commit only source.
  G add package.json
  G commit -q -m init
)
mkdir -p Desktop/atlas
(
  cd Desktop/atlas
  file package.json 1
  file src/index.ts 1
  G init -q -b main
  G add .
  G commit -q -m init
  mkdir -p .claude/worktrees
  G worktree add -q -b wt-clean .claude/worktrees/wt-clean
  G worktree add -q -b wt-dirty .claude/worktrees/wt-dirty
  echo "uncommitted" > .claude/worktrees/wt-dirty/dirty.txt
)

# ---- Idle vs active projects with node_modules ------------------------------
for p in idle-proj active-proj; do
  file Desktop/$p/package.json 1
  file Desktop/$p/package-lock.json 1
  file Desktop/$p/node_modules/dep/index.js 32
done
# Idle: everything last touched long ago. Active: left with fresh mtimes.
find Desktop/idle-proj -exec touch -h -t 202001010000 {} +

echo "fixture ready: $HOME_DIR"
