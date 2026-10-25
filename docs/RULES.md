# DiskLens: Classification and Cleanup Rules

Version 1.0 · Companion to `docs/PRD.md` and `docs/PLAN.md`

This document defines how DiskLens decides what a folder is, which color it gets, whether it can be cleaned, and how. The TOML in section 7 is the starter `rules/rules.toml`. Any change to that file must update this document and its tests in the same commit.

## 1. Principles

1. **Allowlist, not denylist.** Nothing is cleanable unless a rule explicitly matches it.
2. **Defense in depth.** Protected paths live both in `rules.toml` and hardcoded in Rust. The code list wins if they disagree.
3. **Prefer the owning tool.** When a tool has its own cleanup command that is smarter than deleting files (`pnpm store prune`, `ollama rm`, `git worktree remove`), use it.
4. **Explain consequences.** Every rule states what the data is and what happens after removal, in plain language.
5. **Conservative tiers.** If a rule could be either safe or review, it is review.

## 2. Tiers

| Tier | Meaning | Default method | UI |
|------|---------|----------------|----|
| `safe` | The owning tool regenerates it automatically. The only cost is download or rebuild time. | `delete` | "Safe to remove" section |
| `review` | Restorable but costly (large downloads, manual setup), or may hold work the user cares about | `trash` | "Review first" section |
| `never` | User data or system-critical | none | Never offered. Declared with `[[protect]]` |

## 3. Rule schema

Each `[[rule]]` table supports these fields:

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `id` | string | yes | Unique, kebab-case. Used in item IDs and logs |
| `label` | string | yes | Row title |
| `description` | string | yes | "What it is" |
| `regenerates` | string | yes | "What happens if removed" |
| `category` | string | yes | One of the category IDs in section 6 |
| `tier` | `"safe"` or `"review"` | yes | |
| `paths` | string array | yes | Globs. `~` expands to the scan root. `**` matches zero or more segments |
| `exclude` | string array | no | Globs. A path matching any exclude is not a match |
| `target` | `"self"` or `"contents"` | no, default `self` | `contents` empties the folder but keeps it |
| `method` | `"delete"`, `"trash"` or `"command"` | yes | |
| `command` | string array | when `method = "command"` | argv. Placeholders: `{path}`, `{name}`, `{main_repo}` |
| `fallback_method` | `"delete"` or `"trash"` | no | Used when `requires_tool` is missing. Without it, the rule is hidden |
| `provider` | string | no | Custom item enumeration, see section 4.3 |
| `guards` | inline table | no | See section 4.2 |
| `allow_permanent` | bool | no, default `true` | `false` hides the "delete permanently" option for trash items |
| `size_is_estimate` | bool | no, default `false` | UI shows "up to" |
| `min_size_mb` | integer | no, default `1` | Items smaller than this are dropped |
| `priority` | integer | no, default `0` | Higher wins when several rules match one path |
| `split_rows_by` | `"project"` | no | Produces one row per project instead of one per rule |
| `row_label` | string | no | Template for split rows. Supports `{project}` |

Validation runs at startup and in tests, and fails on any of these:

- unknown fields or duplicate IDs
- unknown category
- `command` missing when `method = "command"`, or present otherwise
- unknown guard names or providers
- a glob that doesn't compile

## 4. Matching

### 4.1 Algorithm

Matching runs once per snapshot over directory nodes, in depth-first order from the root.

1. If the node is protected (hardcoded list or `[[protect]]`), stop and don't descend.
2. Collect rules where a `paths` glob matches, no `exclude` glob matches, and all match-time guards pass.
3. If any rules matched:
   1. Pick the highest `priority`. On a tie, pick the rule whose matching glob has the longest literal prefix.
   2. The node becomes an item of that rule, and its subtree is consumed: no descendant can match another rule.
   3. Stop descending.
4. Otherwise, descend into children.

Globs use `globset` with `literal_separator(true)`, matched against the absolute path after expanding `~` (the scan root is escaped, so glob characters in it are literal). Item size is the node's allocated size. With `target = "contents"` the item size is the same, but the action iterates children.

Glob semantics, pinned by tests:

- `*` never crosses `/`.
- `**` matches zero or more whole segments, so `~/**/.next` also matches `~/.next`.
- A trailing `/**` needs at least one more segment. `**/node_modules/**` matches paths inside a `node_modules` folder but not the folder itself, which is what lets `node-modules-idle` match a project's `node_modules` while excluding nested ones.

Inaccessible folders, symlinks and mount points also stop the walk, like protected paths.

### 4.2 Guards

Guards fall into three classes, and each class changes the outcome differently.

| Guard | Value | Class | Behavior |
|-------|-------|-------|----------|
| `sibling_exists` | string array | match-time | Any of these names must exist in the parent folder, otherwise no match |
| `project_idle_days` | integer | match-time | The parent project must be idle this many days, otherwise no match. Idle means the newest of the last commit time, `package.json` mtime and lockfile mtime. Only a git repository below the scan root counts: a repository enclosing the root (a dotfiles repo in home) is ignored. With no evidence at all the project is not idle |
| `requires_tool` | string | match-time | The tool must resolve to a binary. Otherwise `fallback_method` is used, or the rule is hidden |
| `quiet_minutes` | integer | blocking | Blocked if the target or any immediate child was modified within N minutes |
| `process_not_running` | string array | blocking | Blocked if any process command line contains one of these substrings |
| `app_not_running` | string array | blocking | Blocked if an app with one of these bundle IDs is running |
| `owner_app_not_running` | bool | blocking | Treats the folder name as a bundle ID or process name and blocks while it runs |
| `requires_running` | string array | blocking | Blocked unless a process containing one of these substrings is running |
| `git_clean` | bool | safety | Blocked if a worktree has uncommitted or untracked changes, or commits on no branch (a detached HEAD ahead of every local and remote branch). Commits on a local branch do not block: `git worktree remove` keeps the branch |

Blocked items appear in the list with a disabled checkbox and a reason, for example "Quit Xcode to clean", "Start Docker Desktop to clean" or "Has uncommitted changes". A check that cannot complete (git fails, for example) blocks the item rather than allowing it.

### 4.3 Providers

Some data can't be cleaned by deleting a matched folder. A provider replaces the default enumeration with tool-aware items.

In sandbox mode (`DISKLENS_SCAN_ROOT` set) only `git` resolves as a tool. The `ollama`, `simctl_unavailable` and `docker` providers act on machine-wide state, so they return nothing there, and rules that require another tool use their `fallback_method` or are hidden.

| Provider | Enumeration | Item command |
|----------|-------------|--------------|
| `ollama` | Manifests under `<matched folder>/manifests` (no `ollama list`), whether or not the server runs; the `requires_running` guard blocks removal while it is down. One item per model, sized from its blobs. Blobs shared by several models count once, for the first model by name | `ollama rm {name}` |
| `git_worktrees` | Each child of the matched folder whose `.git` is a file containing `gitdir:`. Resolves `{main_repo}` and `{project}` (main repo folder name) | `git -C {main_repo} worktree remove {path}` (never `--force`) |
| `simctl_unavailable` | `xcrun simctl list devices unavailable -j`, sizing each device folder from the snapshot | `xcrun simctl delete unavailable` once for all items |
| `docker` | `docker system df --format json`. Reclaimable size of images, containers and build cache (volumes excluded), always an estimate. When Docker does not answer, the matched folder's size stands in as the upper bound | `docker system prune -a -f` (volumes are kept) |

## 5. Hardcoded protections (Rust, not configurable)

The cleanup engine refuses any target where:

- the canonical path is outside the scan root, or equals the scan root
- the target is one of the standard top-level folders themselves: `~/Desktop`, `~/Documents`, `~/Downloads`, `~/Library`, `~/Pictures`, `~/Movies`, `~/Music`, `~/Public`, `~/Applications`
- the target is a symlink or a mount point
- the target is a directory containing a `.git` directory
- the target is inside any path in the protected list below

The protected list is identical to the `[[protect]]` entries in section 7, plus a unit test asserting both lists match.

## 6. Categories

Categories drive treemap colors. They are resolved for every node, including nodes that no rule matches.

Each category has a one-line `description`. A node's tooltip shows its rule's description when a rule matches it, otherwise its category's description, so every node has a category and a one-line description (PRD goal G2).

**Resolution order:** walk from the node up to the root. At each level, check whether the folder has a project marker (`code_projects`) or equals a category `paths` entry. The first hit wins.

The scan root itself is not checked for project markers, so a dotfiles repository in the home folder does not turn every unclaimed folder into a code project. Nodes inside a matched recommendation take that rule's category.

**Fallback:** if nothing hits, the node is `documents_media` when at least 60% of its bytes are in `media_extensions`, and `dev_tools` otherwise.

## 7. Starter rules (`rules/rules.toml`)

```toml
schema_version = 1

media_extensions = [
  "jpg", "jpeg", "png", "heic", "gif", "webp", "tiff", "raw", "cr2", "nef", "arw", "dng",
  "mov", "mp4", "m4v", "avi", "mkv", "mp3", "m4a", "wav", "aiff", "flac",
  "pdf", "psd", "ai", "sketch", "fig", "key", "pages", "numbers", "docx", "xlsx", "pptx"
]

# ============================================================
# Categories
# ============================================================

[[category]]
id = "code_projects"
label = "Code projects"
description = "Source code, installed dependencies and build output of your projects."
fill = "#E6E4FB"
border = "#8E84E8"
markers = [".git", "package.json", "Cargo.toml", "go.mod", "pyproject.toml", "Package.swift", "pubspec.yaml", "build.gradle", "build.gradle.kts", "composer.json", "Gemfile"]
paths = []

[[category]]
id = "ai_models"
label = "AI models"
description = "Local AI models and the caches of the tools that download them."
fill = "#FBDDE6"
border = "#E58AA7"
markers = []
paths = ["~/.ollama", "~/.cache/huggingface", "~/.lmstudio", "~/.cache/lm-studio"]

[[category]]
id = "mobile_sdks"
label = "iOS & Android SDKs"
description = "Xcode and Android data: SDKs, simulators, emulators and build products."
fill = "#D9EEF4"
border = "#6FB7CC"
markers = []
paths = ["~/Library/Android", "~/.android", "~/Library/Developer"]

[[category]]
id = "package_caches"
label = "Package caches"
description = "Downloads that package managers and apps keep to speed up installs and launches."
fill = "#DCE5F7"
border = "#7C9BD8"
markers = []
paths = ["~/.npm", "~/.gradle", "~/.cache", "~/.bun", "~/.cargo", "~/.yarn", "~/go/pkg", "~/Library/Caches", "~/Library/pnpm"]

[[category]]
id = "documents_media"
label = "Documents & media"
description = "Your documents, photos, videos, music and device backups."
fill = "#FCE6D8"
border = "#EFA57A"
markers = []
paths = ["~/Documents", "~/Pictures", "~/Movies", "~/Music", "~/Downloads", "~/.Trash", "~/Library/Mobile Documents", "~/Library/Application Support/MobileSync"]

[[category]]
id = "dev_tools"
label = "Dev tools & other"
description = "Developer tool settings, app data and anything not in another category."
fill = "#EEEEF0"
border = "#C4C4C9"
markers = []
paths = ["~/.vscode", "~/.cursor", "~/.docker", "~/.claude", "~/.local", "~/.config", "~/Library/Containers/com.docker.docker"]

# ============================================================
# Protected subtrees (mirrored by the hardcoded list in Rust)
# ============================================================

[[protect]]
path = "~/Library/Mobile Documents"
reason = "iCloud Drive"

[[protect]]
path = "~/Library/CloudStorage"
reason = "Cloud provider sync folders"

[[protect]]
path = "~/Library/Keychains"
reason = "Passwords and certificates"

[[protect]]
path = "~/Library/Mail"
reason = "Mail data"

[[protect]]
path = "~/Library/Messages"
reason = "Messages history"

[[protect]]
path = "~/Library/Photos"
reason = "Photos data"

[[protect]]
path = "~/Pictures/*.photoslibrary"
reason = "Photos library"

[[protect]]
path = "~/Library/Application Support/AddressBook"
reason = "Contacts"

[[protect]]
path = "~/Library/Calendars"
reason = "Calendars"

[[protect]]
path = "~/Library/Safari"
reason = "Safari data"

[[protect]]
path = "~/Library/Cookies"
reason = "Cookies"

[[protect]]
path = "~/Library/Preferences"
reason = "App preferences"

[[protect]]
path = "~/Library/Application Support/com.apple.TCC"
reason = "Privacy permissions database"

[[protect]]
path = "~/Library/Containers/com.apple.*"
reason = "Apple app containers"

[[protect]]
path = "~/Library/Group Containers/group.com.apple.*"
reason = "Apple shared containers"

[[protect]]
path = "~/.ssh"
reason = "SSH keys"

[[protect]]
path = "~/.gnupg"
reason = "GPG keys"

[[protect]]
path = "~/.aws"
reason = "Cloud credentials"

[[protect]]
path = "~/.kube"
reason = "Cluster credentials"

# ============================================================
# Safe tier
# ============================================================

[[rule]]
id = "npm-cache"
label = "npm cache"
description = "Package tarballs and metadata npm keeps to speed up installs."
regenerates = "npm downloads packages again on the next install."
category = "package_caches"
tier = "safe"
paths = ["~/.npm/_cacache", "~/.npm/_npx", "~/.npm/_logs"]
method = "delete"
guards = { quiet_minutes = 10 }

[[rule]]
id = "pnpm-store"
label = "pnpm store (unused packages)"
description = "pnpm's shared package store. Packages no project references any more can be pruned."
regenerates = "Pruned packages are downloaded again only if a project needs them."
category = "package_caches"
tier = "safe"
paths = ["~/Library/pnpm/store"]
method = "command"
command = ["pnpm", "store", "prune"]
size_is_estimate = true
guards = { requires_tool = "pnpm", process_not_running = ["pnpm"] }

[[rule]]
id = "bun-cache"
label = "Bun install cache"
description = "Packages Bun downloaded for installs."
regenerates = "Bun downloads packages again on the next install."
category = "package_caches"
tier = "safe"
paths = ["~/.bun/install/cache"]
method = "delete"
guards = { quiet_minutes = 10 }

[[rule]]
id = "yarn-cache"
label = "Yarn cache"
description = "Package archives Yarn keeps between installs."
regenerates = "Yarn downloads packages again on the next install."
category = "package_caches"
tier = "safe"
paths = ["~/Library/Caches/Yarn", "~/.yarn/berry/cache"]
method = "delete"
guards = { quiet_minutes = 10 }

[[rule]]
id = "next-turbo-build"
label = "Next.js & Turbo build output"
description = "Compiled output and build caches from Next.js (.next) and Turborepo (.turbo) in your projects."
regenerates = "Rebuilt on the next dev or build run. The first build afterwards is slower."
category = "code_projects"
tier = "safe"
paths = ["~/**/.next", "~/**/.turbo"]
exclude = ["~/Library/**", "~/.Trash/**", "**/node_modules/**"]
method = "delete"
guards = { sibling_exists = ["package.json", "turbo.json"], quiet_minutes = 15 }

[[rule]]
id = "gradle-caches"
label = "Gradle caches"
description = "Dependencies, build caches and wrapper distributions downloaded by Gradle."
regenerates = "Gradle downloads what it needs on the next build."
category = "package_caches"
tier = "safe"
paths = ["~/.gradle/caches", "~/.gradle/wrapper/dists", "~/.gradle/daemon"]
method = "delete"
guards = { process_not_running = ["GradleDaemon"] }

[[rule]]
id = "xcode-derived-data"
label = "Xcode DerivedData"
description = "Intermediate build products and indexes Xcode generates for each project."
regenerates = "Xcode rebuilds it on the next build. The first build and indexing are slower."
category = "mobile_sdks"
tier = "safe"
paths = ["~/Library/Developer/Xcode/DerivedData"]
target = "contents"
method = "delete"
guards = { app_not_running = ["com.apple.dt.Xcode"], process_not_running = ["xcodebuild"] }

[[rule]]
id = "uv-puppeteer"
label = "uv & Puppeteer caches"
description = "Python packages cached by uv and browser builds downloaded by Puppeteer."
regenerates = "Downloaded again the next time uv installs packages or Puppeteer launches."
category = "package_caches"
tier = "safe"
paths = ["~/.cache/uv", "~/.cache/puppeteer"]
method = "delete"
guards = { process_not_running = ["uv "], quiet_minutes = 10 }

[[rule]]
id = "playwright-browsers"
label = "Playwright browsers"
description = "Browser builds downloaded by Playwright for testing."
regenerates = "Run `npx playwright install` to download them again."
category = "package_caches"
tier = "safe"
paths = ["~/Library/Caches/ms-playwright"]
method = "delete"
guards = { quiet_minutes = 10 }

[[rule]]
id = "pip-cache"
label = "pip cache"
description = "Python packages cached by pip."
regenerates = "pip downloads packages again on the next install."
category = "package_caches"
tier = "safe"
paths = ["~/Library/Caches/pip"]
method = "delete"

[[rule]]
id = "homebrew-cache"
label = "Homebrew downloads"
description = "Installers and old versions Homebrew keeps after upgrades."
regenerates = "Homebrew downloads installers again only when needed."
category = "package_caches"
tier = "safe"
paths = ["~/Library/Caches/Homebrew"]
method = "command"
command = ["brew", "cleanup", "--prune=all"]
fallback_method = "delete"
target = "contents"
size_is_estimate = true
guards = { requires_tool = "brew", process_not_running = ["brew "] }

[[rule]]
id = "cocoapods-cache"
label = "CocoaPods cache"
description = "Pod sources CocoaPods downloaded for iOS projects."
regenerates = "Downloaded again on the next `pod install`."
category = "package_caches"
tier = "safe"
paths = ["~/Library/Caches/CocoaPods"]
method = "delete"

[[rule]]
id = "go-build-cache"
label = "Go build cache"
description = "Compiled package objects the Go toolchain caches."
regenerates = "Rebuilt on the next `go build`."
category = "package_caches"
tier = "safe"
paths = ["~/Library/Caches/go-build"]
method = "delete"

[[rule]]
id = "cargo-registry"
label = "Cargo registry cache"
description = "Crate archives and extracted sources downloaded by Cargo."
regenerates = "Cargo downloads crates again on the next build."
category = "package_caches"
tier = "safe"
paths = ["~/.cargo/registry/cache", "~/.cargo/registry/src"]
method = "delete"
guards = { process_not_running = ["cargo"] }

[[rule]]
id = "app-caches"
label = "App caches"
description = "Caches apps keep in ~/Library/Caches. Apps recreate them as needed."
regenerates = "Apps rebuild their caches on next launch, which may be slower the first time."
category = "package_caches"
tier = "safe"
paths = ["~/Library/Caches/*"]
exclude = ["~/Library/Caches/com.apple.*", "~/Library/Caches/CloudKit", "~/Library/Caches/GeoServices", "~/Library/Caches/familycircled", "~/Library/Caches/PassKit"]
method = "delete"
min_size_mb = 50
priority = -10
guards = { owner_app_not_running = true, quiet_minutes = 5 }

# ============================================================
# Review tier
# ============================================================

[[rule]]
id = "android-sdk-emulators"
label = "Android SDK & emulators"
description = "The Android SDK, emulator system images and virtual devices."
regenerates = "Reinstall from Android Studio's SDK Manager and recreate virtual devices. Emulator data is lost."
category = "mobile_sdks"
tier = "review"
paths = ["~/Library/Android/sdk", "~/.android/avd", "~/.android/cache"]
method = "trash"
guards = { app_not_running = ["com.google.android.studio"], process_not_running = ["qemu-system"] }

[[rule]]
id = "ollama-models"
label = "Ollama models"
description = "Local language models downloaded with Ollama."
regenerates = "Run `ollama pull <model>` to download a model again."
category = "ai_models"
tier = "review"
paths = ["~/.ollama/models"]
provider = "ollama"
method = "command"
command = ["ollama", "rm", "{name}"]
guards = { requires_tool = "ollama", requires_running = ["ollama"] }

[[rule]]
id = "huggingface-cache"
label = "Hugging Face models"
description = "Models and datasets cached by Hugging Face libraries."
regenerates = "Downloaded again the next time code loads them."
category = "ai_models"
tier = "review"
paths = ["~/.cache/huggingface/hub"]
method = "trash"
guards = { quiet_minutes = 30 }

[[rule]]
id = "lmstudio-models"
label = "LM Studio models"
description = "Local models downloaded in LM Studio."
regenerates = "Download them again from LM Studio's model browser."
category = "ai_models"
tier = "review"
paths = ["~/.lmstudio/models", "~/.cache/lm-studio/models"]
method = "trash"
guards = { process_not_running = ["LM Studio"] }

[[rule]]
id = "node-modules-idle"
label = "node_modules in idle projects"
description = "Installed dependencies in projects untouched for 30 days or more."
regenerates = "Run your package manager's install command in the project to restore them."
category = "code_projects"
tier = "review"
paths = ["~/**/node_modules"]
exclude = ["~/Library/**", "~/.Trash/**", "**/node_modules/**", "~/.npm/**", "~/.bun/**", "~/.nvm/**", "~/.vscode/**", "~/.cursor/**", "~/.local/**"]
method = "delete"
guards = { sibling_exists = ["package.json"], project_idle_days = 30, quiet_minutes = 60 }

[[rule]]
id = "agent-worktrees"
label = "Agent worktrees"
description = "Git worktrees created by coding agents for parallel tasks."
regenerates = "Removed with `git worktree remove`. Branches stay in the main repository."
category = "code_projects"
tier = "review"
paths = ["~/**/.claude/worktrees", "~/**/.worktrees"]
exclude = ["~/Library/**", "~/.Trash/**", "**/node_modules/**"]
provider = "git_worktrees"
method = "command"
command = ["git", "-C", "{main_repo}", "worktree", "remove", "{path}"]
split_rows_by = "project"
row_label = "{project} worktrees"
guards = { requires_tool = "git", git_clean = true, quiet_minutes = 60 }

[[rule]]
id = "xcode-archives"
label = "Xcode archives"
description = "App builds archived for distribution. Needed to symbolicate crash reports for those versions."
regenerates = "Cannot be regenerated exactly. Keep archives for versions you still support."
category = "mobile_sdks"
tier = "review"
paths = ["~/Library/Developer/Xcode/Archives"]
target = "contents"
method = "trash"
guards = { app_not_running = ["com.apple.dt.Xcode"] }

[[rule]]
id = "ios-device-support"
label = "Device support files"
description = "Debug symbols Xcode copies from each iOS, watchOS, tvOS and visionOS version you connect."
regenerates = "Xcode copies them again the next time that device version is connected."
category = "mobile_sdks"
tier = "review"
paths = ["~/Library/Developer/Xcode/iOS DeviceSupport", "~/Library/Developer/Xcode/watchOS DeviceSupport", "~/Library/Developer/Xcode/tvOS DeviceSupport", "~/Library/Developer/Xcode/visionOS DeviceSupport"]
target = "contents"
method = "delete"
guards = { app_not_running = ["com.apple.dt.Xcode"] }

[[rule]]
id = "simulators-unavailable"
label = "Unavailable simulators"
description = "Simulator devices for runtimes that are no longer installed."
regenerates = "These simulators can't run any more. Create new ones in Xcode if needed."
category = "mobile_sdks"
tier = "review"
paths = ["~/Library/Developer/CoreSimulator/Devices"]
provider = "simctl_unavailable"
method = "command"
command = ["xcrun", "simctl", "delete", "unavailable"]
guards = { requires_tool = "xcrun", app_not_running = ["com.apple.iphonesimulator", "com.apple.dt.Xcode"] }

[[rule]]
id = "docker-unused"
label = "Docker unused images & cache"
description = "Stopped containers, unused images, networks and build cache in Docker Desktop. Volumes are kept."
regenerates = "Images are pulled or rebuilt again when needed."
category = "dev_tools"
tier = "review"
paths = ["~/Library/Containers/com.docker.docker/Data/vms"]
provider = "docker"
method = "command"
command = ["docker", "system", "prune", "-a", "-f"]
size_is_estimate = true
guards = { requires_tool = "docker", requires_running = ["com.docker.backend"] }

[[rule]]
id = "ios-backups"
label = "iPhone & iPad backups"
description = "Local device backups made by Finder."
regenerates = "Cannot be regenerated. Make a new backup from Finder if needed."
category = "documents_media"
tier = "review"
paths = ["~/Library/Application Support/MobileSync/Backup/*"]
method = "trash"
allow_permanent = false

[[rule]]
id = "trash-bin"
label = "Trash"
description = "Files you already moved to the Trash."
regenerates = "Permanently deleted. This cannot be undone."
category = "documents_media"
tier = "review"
paths = ["~/.Trash"]
target = "contents"
method = "delete"
```

## 8. Adding or changing a rule

1. Add or edit the `[[rule]]` in `rules/rules.toml` and copy the change into section 7 of this document.
2. Choose the tier conservatively (principle 5).
3. Add fixture paths to `scripts/make-fixture.sh`.
4. Add table-driven tests in `tests/rules.rs`:
   - at least one path that matches
   - one that must not match (an exclude or a guard)
   - an overlap test if the rule shares a parent with another rule
5. Run `cargo test` and confirm validation passes.
