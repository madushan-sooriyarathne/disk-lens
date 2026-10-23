#!/bin/bash
# Builds the fixture sandbox at <repo>/.sandbox/home. Idempotent: wipes and rebuilds
# only that directory. Never touches the real home folder.
#
# Rule targets are at least 1 MB (50 MB for app-caches) so they survive each
# rule's min_size_mb, and everything is aged so quiet_minutes guards pass;
# tests/sandbox_rows.rs asserts the rows this produces.
set -euo pipefail

# Bump when the fixture changes; sandbox_rows.rs refuses a stale fixture.
FIXTURE_VERSION=3

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

# file <path> [kb]: create a real file (default 4 KB), creating parents.
file() { mkdir -p "$(dirname "$1")"; head -c "$(( ${2:-4} * 1024 ))" /dev/zero | tr '\0' 'x' > "$1"; }
G() { git -c user.name=fixture -c user.email=fixture@example.com -c commit.gpgsign=false "$@"; }
MB=1100    # just over 1 MB (decimal), the default min_size_mb
BIG=52000  # just over 50 MB, app-caches' min_size_mb

# ---- Safe tier targets -------------------------------------------------------
file .npm/_cacache/content-v2/a.bin $MB
file .npm/_npx/x/index.js $MB
file .npm/_logs/debug.log $MB
file Library/pnpm/store/v3/files/ab/cd $MB
file .bun/install/cache/pkg/index.js $MB
file Library/Caches/Yarn/v6/pkg.tgz $MB
file .yarn/berry/cache/pkg.zip $MB
file Desktop/web-app/package.json 1
file Desktop/web-app/.next/cache/a.js $MB
file Desktop/web-app/.turbo/log.txt $MB
file Desktop/web-app/node_modules/dep/index.js
file .gradle/caches/modules-2/a.jar $MB
file .gradle/wrapper/dists/gradle-8/bin $MB
file .gradle/daemon/8.0/daemon.log $MB
file Library/Developer/Xcode/DerivedData/App-abc/Build/out.o $MB
file .cache/uv/wheel.whl $MB
file .cache/puppeteer/chrome/chrome.bin $MB
file Library/Caches/ms-playwright/chromium-1/chrome $MB
file Library/Caches/pip/http/a $MB
file Library/Caches/Homebrew/downloads/a.tar.gz $MB
file Library/Caches/CocoaPods/Pods/a $MB
file Library/Caches/go-build/aa/obj $MB
file .cargo/registry/cache/index/crate.crate $MB
file .cargo/registry/src/index/crate/lib.rs $MB
file Library/Caches/com.example.app/cache.db $BIG
file Library/Caches/com.example.small/cache.db 64    # app cache under 50 MB: dropped
# ---- Review tier targets -----------------------------------------------------
file Library/Android/sdk/platforms/android-34/android.jar $MB
file .android/avd/Pixel.avd/userdata.img $MB
file .android/cache/a $MB
file .cache/huggingface/hub/models--x/blob $MB
file .lmstudio/models/m/model.gguf $MB
file Library/Developer/Xcode/Archives/2024-01-01/App.xcarchive/Info.plist $MB
file "Library/Developer/Xcode/iOS DeviceSupport/17.0/Symbols/a" $MB
file Library/Developer/CoreSimulator/Devices/AAAA-BBBB/data/a $MB
file Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw $MB
file "Library/Application Support/MobileSync/Backup/0000-1111/Info.plist" $MB
file .Trash/old-file.txt $MB

# Ollama: llama3:8b and llama3:latest share their blobs; qwen has its own.
manifest() { # manifest <path> <digest>...
  local path="$1"; shift
  local layers="" d
  for d in "$@"; do layers="$layers${layers:+,}{\"mediaType\":\"application/vnd.ollama.image.model\",\"digest\":\"sha256:$d\",\"size\":1}"; done
  mkdir -p "$(dirname "$path")"
  printf '{"schemaVersion":2,"config":{"digest":"sha256:c0ffee","size":1},"layers":[%s]}\n' "$layers" > "$path"
}
manifest .ollama/models/manifests/registry.ollama.ai/library/llama3/8b aaaa bbbb
manifest .ollama/models/manifests/registry.ollama.ai/library/llama3/latest aaaa bbbb
manifest .ollama/models/manifests/registry.ollama.ai/library/qwen/0.5b cccc
file .ollama/models/blobs/sha256-aaaa $MB
file .ollama/models/blobs/sha256-bbbb 64
file .ollama/models/blobs/sha256-cccc $MB
file .ollama/models/blobs/sha256-c0ffee 4

# ---- Near misses (must NOT match) -------------------------------------------
file Desktop/not-a-project/.next/a.js $MB           # .next without package.json/turbo.json
file Desktop/web-app/node_modules/dep/node_modules/nested/index.js  # nested node_modules
file Library/Caches/com.apple.Safari/a $MB          # excluded by app-caches
file Library/Caches/CloudKit/a $MB                  # excluded by app-caches
file Library/Developer/Xcode/DerivedData.bak/a $MB  # near-miss name

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
  file .next/cache/a.js $MB
  G init -q -b main
  # .next is untracked build output; commit only source.
  G add package.json
  G commit -q -m init
)
# atlas has four agent worktrees. wt-clean is pushed and wt-local has an
# unpushed commit on its branch: both are ready, because git worktree remove keeps
# the branch. wt-dirty has uncommitted changes and wt-detached has a commit on no
# branch: both are blocked.
G init -q --bare "$OUTSIDE/atlas-remote.git"
mkdir -p Desktop/atlas
(
  cd Desktop/atlas
  file package.json 1
  file src/index.ts 1
  file assets/sprite.bin $MB
  G init -q -b main
  G add .
  G commit -q -m init
  G remote add origin "$OUTSIDE/atlas-remote.git"
  G push -q origin main
  mkdir -p .claude/worktrees
  G worktree add -q -b wt-clean .claude/worktrees/wt-clean
  G worktree add -q -b wt-dirty .claude/worktrees/wt-dirty
  G worktree add -q -b wt-local .claude/worktrees/wt-local
  G worktree add -q --detach .claude/worktrees/wt-detached
  G -C .claude/worktrees/wt-clean push -q origin wt-clean
  echo "local" > .claude/worktrees/wt-local/local.txt
  G -C .claude/worktrees/wt-local add local.txt
  G -C .claude/worktrees/wt-local commit -q -m "local work"
  echo "detached" > .claude/worktrees/wt-detached/detached.txt
  G -C .claude/worktrees/wt-detached add detached.txt
  G -C .claude/worktrees/wt-detached commit -q -m "detached work"
  echo "uncommitted" > .claude/worktrees/wt-dirty/dirty.txt
)

# ---- Idle vs active projects with node_modules ------------------------------
for p in idle-proj active-proj; do
  file Desktop/$p/package.json 1
  file Desktop/$p/package-lock.json 1
  file Desktop/$p/node_modules/dep/index.js $MB
done

# Everything was last touched long ago, so quiet_minutes guards pass and
# projects count as idle. Only active-proj is touched again afterwards.
find . -exec touch -h -t 202401010000 {} +
find Desktop/active-proj -exec touch -h {} +

echo "$FIXTURE_VERSION" > "$SANDBOX/fixture-version"
echo "fixture ready: $HOME_DIR"
