# DiskLens: Implementation Plan

Version 2.0 draft · Companion to `docs/PRD.md` and `docs/RULES.md`

## Status and roadmap (as of 2026-10-26)

| Phase | State | Notes |
|-------|-------|-------|
| 0 Scaffold | Done | |
| 1 Scanner | Done | Within 1% of `du -skx`; full home scan of 3.8M files in about 31 s, peak about 444 MB |
| 2 Explorer UI | Done, approved | |
| 3 Rules engine and recommendations | Done, awaiting review | All automated checks pass. The UI review moves to the GPUI build (Phase 3.5) |
| 3.5 GPUI migration | Done, merged | Interactions still need a check in the native window (see What is left) |
| 4 Cleanup engine | Done, verified on macOS | `docs/QA.md` run on 2026-10-25, including the supervised npm run; open findings under What is left |
| 5 Permissions, persistence, polish | Done, awaiting review | Built on branch `feat/phase-5-polish`; PRD section 10 targets measured and met. Manual checks in `docs/QA.md` (Phase 5) still to run, including the fresh-account onboarding |
| 6 Packaging and release | Built, awaiting review | Branch `feat/phase-6-release` (stacked on Phase 5). Universal ad-hoc signed DMG, icon, updater. Developer ID signing, notarization and the macOS 13 launch are open (see What is left) |
| 7 Background rescan | Built, verified on macOS, awaiting review | v2, PRD section 13.4.2. Branch `feat/phase-7-background-rescan`; `docs/QA.md` Phase 7 checks passed on 2026-10-31 |
| 8 Design foundation and window shell | Not started | v2, PRD sections 13.4.3 and 13.5 |
| 9 Overview | Not started | v2, PRD section 13.4.4 |
| 10 Explore and inspector | Not started | v2, PRD sections 13.4.5 and 13.4.7 |
| 11 Recommended and cleanup sheet | Not started | v2, PRD sections 13.4.6 and 13.4.8 |
| 12 Onboarding | Not started | v2, PRD section 13.4.1 |
| 13 Command palette | Not started | v2, PRD section 13.4.9 (free part) |
| 14 Active-work guard and project artifacts | Not started | v2, PRD section 13.4.11 (free) |
| 15 Pro licensing and gating | Not started | v2, PRD section 13.6; needs the open decisions in PRD section 13.8 |
| 16 Lockfile tags and orphaned stores | Not started | Pro, PRD section 13.10.1 |
| 17 APFS snapshot inspector | Not started | Pro, PRD section 13.10.5 |
| 18 Containers and VMs | Not started | Pro, PRD section 13.10.3 |
| 19 AI models hub | Not started | Pro, PRD section 13.10.2 |
| 20 Apple, mobile and creative scanners | Not started | Pro, PRD section 13.10.4 |
| 21 Menu bar agent | Not started | Pro, PRD section 13.10.6 |
| 22 Auto-sweep | Not started | Pro, PRD section 13.10.7; needs Phase 21 |
| 23+ Remaining Pro features | Not started | Other volumes, history, large and old files, duplicates, local rules file, clone-aware sizes; one phase each |

Last full gate run on macOS (Phase 7, 2026-10-31): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (346 tests, 1 release-only test ignored) all pass.

### What is left

**Phase 3.5 review**
- Check in the native window what screenshots could not: drill-down, breadcrumb, Esc (closes the menu, then the drawer, then goes up), mode toggle, Rescan, tile tooltip, right-click menu, row and item checkboxes, Select safe, drawer open and close, Reveal in Finder, window resize, scan progress animation, and the idle screen.
- Mockup differences, decided: keep "92.0 GB" (the formatter shows one decimal below 100) and "up to 9.0 GB" (RULES.md section 3 says estimates read "up to"); the mockup's "92 GB" and "~9.0 GB" are not followed. Check marks and arrows stay text glyphs.
- Done: the drawer shows the error when details fail to load.

**Phase 4: cleanup engine** (section 5, Phase 4)
- Built: `cleanup/` (`model`, `protected` refusals, `plan`, `revalidate`, `execute`, `audit`, `script`), the backend functions, and `src/ui/cleanup.rs` (confirm, progress, results, toast). Copy script and Clean selected are enabled.
- Decisions: after a cleanup the whole scan root is rescanned (not only the parent folders); "inside home" means inside the scan root, so sandboxed runs work; sandboxed audit logs go to `<scan root>/../logs` (`.sandbox/logs`).
- Verified in the Linux session: the cleanup unit tests, `tests/cleanup_adversarial.rs`, `tests/cleanup_engine.rs`, `tests/cleanup_backend.rs` and `tests/script.rs` pass on a Linux build with the UI and macOS calls stubbed out; a full real cleanup of `.sandbox` (31 ready items, Trash redirected into `.sandbox/`) removed every ready item, left blocked and protected paths alone, and the audit log had one line per item.
- Verified on macOS (2026-10-25): the full gate, the native-window checks in `docs/QA.md` (Phase 3.5 interactions included), trashing through `NSFileManager`, and the supervised npm run (freed size matched `du` exactly). Results are in `docs/QA.md`.
- Open: Finder's Put Back is not offered for items trashed with `NsFileManager` (decide: Finder method or drop the check); toggling permanent deletion re-renders the confirm dialog; a mount-point refusal is still untested (cannot be built in a test).

**Phase 5: permissions, persistence, polish**
- Built:
  - Full Disk Access: `platform::full_disk_access` opens `TCC.db` under the real home. The backend has `check_permissions` and `open_fda_settings`.
  - Onboarding: `Phase::Onboarding` in `src/ui/model.rs`, with the card in `src/ui/screens.rs`. Access is rechecked on every window activation. The unreadable-folders banner offers "Grant Full Disk Access" while access is missing.
  - Settings: `src/settings.rs` writes `settings.json` in the data dir. Its only field is `onboarding_done`.
  - "Scanned X ago" before Rescan (`format_age` in `src/ui/format.rs`).
  - G2: each `[[category]]` has a `description`. The tile tooltip shows the matching rule's description, otherwise the category's.
  - Dark mode: `Theme::dark()` follows `window.appearance()` and repaints when the appearance changes. Category colors come from `fill_dark` and `border_dark` in `rules.toml`. Validation rejects colors that are not `#RRGGBB`.
  - Keyboard: Tab and Shift-Tab reach tiles, header controls, breadcrumb links, checkboxes, the chevrons and buttons. Enter and Space click the focused element. A focus ring shows for keyboard focus only.
  - Accessibility: controls carry AccessKit roles and labels, and the macOS adapter is wired in `gpui-pre-macos`.
  - Cleanup safety: the cleanup confirm button is not a Tab stop, so Enter never confirms (PRD 7.6).
- Measured (2026-10-25, Apple Silicon, release build):
  - `scripts/make-perf-tree.sh` builds 2,000,000 empty files in 202,121 folders.
  - `scan-bench` peak resident memory: 189 MB (target 400 MB). Scan time about 20 s.
  - Treemap layout of 420 tiles: under 1 ms (target 16 ms), from `cargo test --release -- --ignored layout_of_400_tiles`.
  - No optimisation was needed.
  - The earlier real 3.8M-file home scan (about 444 MB) had more files than the 2M target.
- Open:
  - Run the Phase 5 checks in `docs/QA.md`: onboarding on a fresh macOS user account, VoiceOver reading the labels, keyboard navigation in the native window, and dark mode.
  - Contrast: the PRD's tier colors fail WCAG AA as text in light mode. Safe green on its pill fill is 2.9:1, and review amber on its fill is 2.0:1; blocked reasons are in amber on white. Decide whether to darken the light text shades (fills and hatches can stay) or accept it. Dark mode passes, and the tests cover the text and muted tokens in both themes.

**Phase 6: packaging and release**
- Decisions (2026-10-25): bundle identifier `com.madushan.disklens`; ad-hoc signing until a Developer ID certificate exists; signed auto-updates are part of v1; the icon is `assets/icons/icon.svg` (glassy treemap with a lens).
- Built:
  - Version 1.0.0. `[package.metadata.packager]` in `Cargo.toml` (cargo-packager 0.11.8), the x86_64 target in `rust-toolchain.toml`, and `MACOSX_DEPLOYMENT_TARGET = 13.0` in `.cargo/config.toml`.
  - `scripts/package.sh`: builds both slices, joins them with `lipo`, and runs `cargo packager` for `DiskLens.app` and `DiskLens_<version>_universal.dmg` in `target/packager`. Hardened runtime, ad-hoc unless `DISKLENS_SIGNING_IDENTITY` is set; cargo-packager notarizes and staples when the `APPLE_API_*` variables are set. With the minisign key set it also writes `DiskLens.app.tar.gz`, its `.sig` and `latest.json`, then prints the `gh release create --draft` command without running it.
  - `scripts/make-icon.sh` regenerates `icon.icns` from `icon.svg` with tools that ship with macOS.
  - `src/update.rs` and an update banner: one check per launch against `releases/latest/download/latest.json`, Install, then Restart. Not offered during a scan or cleanup.
- Measured (2026-10-25): DMG 9.0 MB with the updater (target 25 MB); universal binary 15.3 MB; the updater adds about 0.9 MB to the arm64 binary.
- Open:
  - Developer ID signing and notarization: needs an Apple Developer Program membership, a Developer ID Application certificate and a notarytool API key. Until then PRD section 12 ("signed, notarized") is not met.
  - Ad-hoc consequences: macOS ties the Full Disk Access grant to the build, so users grant access again after each update; on macOS 15 and later the first launch needs Open Anyway in Privacy & Security. Say both in the release notes.
  - Launch on a clean macOS 13 machine (PRD section 12), for example a UTM virtual machine.
  - Run the Phase 6 checks in `docs/QA.md`, then publish the first draft release when the user says so.
  - The update banner shows only on the main (ready) screen, so a user still on onboarding or the idle screen sees it after the first scan. An install that fails is not retried until the next launch.
  - Not verified on a real release: the signed download, install and `cx.restart()` relaunch. `docs/QA.md` has a local round trip for it.
  - `create-dmg` sometimes fails with `hdiutil: couldn't unmount ... Resource busy` when Finder holds the volume; detach it (`hdiutil detach <dev> -force`), delete `target/packager/rw.*.dmg` and run `scripts/package.sh` again.

**Phase 7: background rescan**
- Built (all in `src/ui/`):
  - `AppModel.scan: Option<ScanProgress>` holds the running scan. `Phase::Scanning` is used only while no snapshot exists; a rescan keeps `Phase::Ready`, and `scan` stays set until the new snapshot is on screen, so Clean selected cannot build a plan against IDs that are about to change.
  - Snapshot swap (`AppModel::enter`): the explorer path falls back to the nearest ancestor in the new snapshot (`ancestors`), the selection is carried over by `(rule_id, path)` (`selection::carry_over`) with a toast counting dropped items, and the drawer stays open (details reloaded) only if its row still exists. A navigation made while the swap loads wins over the carried-over path.
  - Clean selected is disabled during a scan with the tooltip "Available when the update finishes"; `open_cleanup` and `confirm_cleanup` refuse while scanning. Copy script, selection and browsing keep working.
  - A snapshot older than 24 hours (`STALE_AFTER`) starts a background rescan when it is first shown after launch.
  - Deleted or trashed items leave the recommendations, the selection and the treemap sizes as each one finishes (`selection::forget_item`, `subtract`), before the follow-up rescan.
  - Explorer header: "Updating… N files" with Cancel; the failure or cancel message with Retry; otherwise "Scanned X ago" with Rescan. The banner above the stat cards now only shows the unreadable-folders notice.
- Decisions:
  - Retry takes Rescan's place while an error shows (both start the same scan), instead of a second button next to it.
  - Dry-run, skipped, failed and command items stay on screen until the rescan: PRD FR-BG-6 names deleted or trashed items, and a command frees an estimated size from a folder that still exists.
  - "Other" tiles are not shrunk by `subtract`: they carry their folder's path and only stand for leftovers.
- Verified (2026-10-31): the Phase 7 checks in `docs/QA.md` passed in the native window on the sandbox.
- Open:
  - If loading the new snapshot fails after a background scan, the backend already holds the new snapshot while the UI keeps the old view, with the error and Retry; Retry rescans.

**Housekeeping**
- `platform/` is one `mod.rs` with the `System`, `Runner` and `Trash` traits. Phase 5 adds FDA code there.

### v2

PRD section 13 defines v2: onboarding, background rescans, a HIG-based redesign on GPUI, a simpler treemap with semantic textures, a keyboard-first workflow (command palette, Vim keys, staging queue), a free active-work git guard, and a free/Pro split. Pro features are specified in PRD section 13.10. The v1 candidates (large and old files, duplicates, clone-aware sizes, menu bar indicator, scheduled cleanup, local rules file) became Pro features; the menu bar indicator grew into the menu bar agent, and scheduled cleanup became auto-sweep (stage and notify, never unattended removal: PRD S-11). Phases 7 to 22 are in section 5. Phases 16 to 22 follow the recommended order in PRD section 13.8 and move if that decision changes.

Decisions (2026-10-29): the active-work guard, staging queue and Copy as Shell Script are free; auto-sweep only stages and notifies; admin rights are opt-in, one action at a time, for snapshot actions only (the S-06 exception); in Explore, Space stages and Quick Look is on ⌘Y. The Phase 6 open items (Developer ID, notarization, the macOS 13 launch) still apply and block selling Pro.

## 0. Working agreement

- The three documents in `docs/` are the source of truth. If they conflict or something is unclear, ask before guessing.
- Work one phase at a time. Each phase ends with its acceptance checks, a short summary, and a stop for review.
- **Never run cleanup against the real home folder during development.**
  - Use the fixture sandbox (`DISKLENS_SCAN_ROOT=./.sandbox/home`).
  - Debug builds default to `DISKLENS_DRY_RUN=1`.
  - Real execution is enabled only in Phase 4, on one safe item, with the user present.
- Use the latest stable versions of every dependency, and confirm configuration against official docs rather than memory.
- Commit per task using conventional commit messages.

## 1. Stack

| Layer | Choice |
|-------|--------|
| Platform | macOS only, minimum macOS 13 |
| Language | Rust stable (pinned in `rust-toolchain.toml`), edition 2024. One crate at the repo root |
| UI | GPUI, Zed's GPU UI framework: `gpui-pre` and `gpui-pre-platform` pinned to an exact snapshot version (`=0.3.9`), imported as `gpui` and `gpui_platform`. v1: no component library. v2: `gpui-component` from gpui-kit (0.8.1, which pins `gpui-pre =0.3.10`, so GPUI is bumped first, in its own commit) for the palette, text input, lists, menus, popovers and tooltips, themed with DiskLens tokens. Any component that can't follow the HIG is hand-built |
| Styling | Theme tokens in `src/ui/theme.rs`; category colors come from `rules.toml` |
| State | One GPUI entity, `AppModel` (`src/ui/model.rs`) |
| Treemap | Squarified layout ported from d3 into `src/ui/treemap.rs`, rendered as absolutely positioned divs |
| Fonts | v1: Geist and Geist Mono static TTFs (OFL) embedded from `assets/fonts/`. v2: the system font (`.SystemUIFont`) with tabular figures; Geist Mono kept only as the path fallback (Phase 8) |
| Motion (v2) | GPUI's `spring.rs` and `with_animation`; no extra crate |
| Tooling | cargo (`fmt`, `clippy`, `test`) |
| Packaging | `cargo-packager` CLI 0.11.8 (`.app`, DMG, codesign, notarization, minisign update signatures) |

**Rust crates**

| Purpose | Crates |
|---------|--------|
| UI | `gpui`, `gpui_platform` |
| Serialization | `serde`, `serde_json`, `toml`, `zstd` |
| Rule matching | `globset` |
| Scanning | `jwalk`, `rayon`, `libc` |
| Processes | `sysinfo` |
| Trash | `trash` (macOS `DeleteMethod::NsFileManager`) |
| macOS APIs | `objc2`, `objc2-foundation`, `objc2-app-kit` (running apps, volume capacity) |
| Commands | `wait-timeout`, `shell-escape` (script export) |
| Updates | `cargo-packager-updater` (`native-tls`) |
| Logging | `tracing`, `tracing-appender` |
| Errors | `thiserror`, `anyhow` |
| Tests | `tempfile` |
| v2: git state | `git2` (libgit2, vendored) for the active-work guard |
| v2: fuzzy search | `nucleo-matcher` (command palette) |
| v2: macOS APIs | more `objc2-app-kit` (status item, dragging source, Quick Look panel), `objc2-service-management` (`SMAppService` login item) |

Git operations shell out to the `git` binary in v1. From Phase 14 the active-work check reads repositories in process with `git2`; worktree removal still runs `git worktree remove` (the owning tool). The Docker API (Phase 18) is a plain HTTP GET over `std::os::unix::net::UnixStream`, with no client crate.

## 2. Repository layout

```
disklens/
  Cargo.toml, rust-toolchain.toml, CLAUDE.md
  docs/                 PRD.md, RULES.md, PLAN.md, QA.md
  scripts/
    make-fixture.sh     builds .sandbox/home with realistic and adversarial cases
    make-perf-tree.sh   2M empty files in .sandbox/perf
    package.sh          universal .app, DMG and update artifacts in target/packager
    make-icon.sh        assets/icons/icon.icns from icon.svg
  rules/rules.toml
  assets/
    fonts/              Geist, Geist Mono (OFL)
    mock/snapshot.mock.json   matches the mockup numbers (DISKLENS_MOCK)
    icons/              icon.svg (source), icon.icns (generated)
    update.pub          minisign public key for updates
  src/
    main.rs, lib.rs, backend.rs, state.rs, error.rs, config.rs, view.rs, settings.rs, update.rs
    scanner/  walk.rs, tree.rs, hardlinks.rs, volume.rs, snapshot.rs
    rules/    model.rs, load.rs, validate.rs, category.rs, matcher.rs, guards.rs, tools.rs
    providers/ ollama.rs, git_worktrees.rs, simctl.rs, docker.rs
    recommend/ build.rs, model.rs
    cleanup/  model.rs, protected.rs, plan.rs, revalidate.rs, execute.rs, script.rs, audit.rs
    platform/ mod.rs
    ui/       mod.rs, root.rs, model.rs, source.rs, theme.rs, format.rs, stats.rs,
              selection.rs, treemap.rs, screens.rs, stat_cards.rs, explorer.rs,
              recommendations.rs, cleanup.rs
    bin/scan-bench.rs
    v2 adds: ui/chrome.rs, ui/palette.rs, ui/keymap.rs, ui/queue.rs, ui/inspector.rs,
             rules/git_state.rs, providers/{store_orphans,containers,models,simruntimes}.rs,
             snapshots.rs (APFS), license.rs, sweep.rs, bin/disklens-agent.rs
  tests/      scanner.rs, rules.rs, rules_doc_sync.rs, sandbox_rows.rs, view.rs,
              cleanup_adversarial.rs, cleanup_engine.rs, cleanup_backend.rs, script.rs,
              support/mod.rs (temporary scan roots under target/tmp)
```

## 3. Data contracts

Rust types derive `Serialize` and `Deserialize` (snapshot persistence and the mock). The UI uses these types directly; there is no separate frontend contract.

```rust
pub struct VolumeInfo { pub total: u64, pub available: u64 }

pub struct DirNode {
    pub id: u32, pub parent: Option<u32>, pub name: String,
    pub size: u64, pub logical_size: u64, pub file_count: u64,
    pub children: Vec<u32>, pub top_files: Vec<FileLeaf>,
    pub flags: NodeFlags, pub category: String,
}
pub struct NodeFlags { pub inaccessible: bool, pub symlink: bool, pub mount: bool }
pub struct FileLeaf { pub name: String, pub size: u64 }

pub struct SnapshotMeta {
    pub id: String, pub root: String, pub scanned_at: i64, pub duration_ms: u64,
    pub volume: VolumeInfo, pub scanned_size: u64, pub inaccessible_count: u32,
}

pub enum ViewMode { Category, Cleanup }
pub struct ViewNode {
    pub id: String, pub name: String, pub path: String, pub size: u64,
    pub category: String, pub tier: Option<Tier>, pub rule_id: Option<String>,
    pub kind: ViewKind, // Dir | File | Other
    pub children: Vec<ViewNode>,
}

pub enum Tier { Safe, Review }
pub struct Recommendation {
    pub row_id: String, pub rule_id: String, pub label: String, pub paths_hint: String,
    pub tier: Tier, pub size: u64, pub size_is_estimate: bool, pub items: Vec<RecItem>,
}
pub struct RecItem {
    pub item_id: String, pub path: String, pub name: String, pub size: u64,
    pub status: ItemStatus, // Ready | Blocked { reason }
    pub method: MethodView, // Delete | Trash | Command { display }
}

pub struct CleanupPlan {
    pub plan_id: String, pub snapshot_id: String, pub actions: Vec<PlannedAction>,
    pub total: u64, pub trash_total: u64, pub permanent_available: bool,
    pub warnings: Vec<String>, // requested items left out, with the reason
}
pub struct PlannedAction {
    pub item_id: String, pub rule_id: String, pub label: String, pub name: String,
    pub tier: Tier, pub path: String, pub matched: String, pub size: u64,
    pub size_is_estimate: bool, pub measurable: bool,
    pub method: PlannedMethod, // Delete | DeleteContents | Trash | TrashContents | Command { argv }
}

pub enum ScanEvent { Progress { files: u64, bytes: u64, current: String }, Finished { meta: SnapshotMeta }, Failed { message: String } }
pub enum CleanupEvent { ItemStarted { item_id: String }, ItemFinished { item_id: String, freed: u64, outcome: Outcome }, Finished { summary: CleanupSummary } }
pub enum Outcome { Deleted, Trashed, CommandRan, DryRun, Skipped { reason: String }, Failed { error: String } }
pub struct CleanupSummary { pub freed: u64, pub trashed: u64, pub done: u32, pub skipped: Vec<SkippedItem>, pub dry_run: bool }
```

Item IDs are `"{rule_id}:{node_id}"` (or `"{rule_id}:{provider_key}"` for providers). They are valid only for the snapshot that produced them, so `build_cleanup_plan` also takes the snapshot ID the selection was made in and refuses any other.

**v2 changes**

```rust
// ViewMode is removed: "Highlight cleanable" is a UI-only dimming (PRD 13.4.5).
pub struct ViewNode {
    pub id: String, pub name: String, pub path: String, pub size: u64,
    pub category: String, pub tier: Option<Tier>, pub rule_id: Option<String>,
    pub cleanable: u64,         // ready recommended bytes at or under this node
    pub system_managed: bool,   // protected, or under ~/Library with nothing recommended inside
    pub kind: ViewKind,         // Dir | File | Smaller { count: u32 } (was Other)
    pub children: Vec<ViewNode>, // root: up to 12 tiles plus Smaller; Smaller: the merged items; others: empty
}

pub struct CategoryTotal { pub category: String, pub size: u64, pub folders: u32 }

pub struct ScanProgress { pub files: u64, pub bytes: u64, pub current: String } // UI model only

pub struct LicenseStatus { pub pro: bool, pub licensee: Option<String>, pub expires_at: Option<i64> }

// Phase 14 onward
pub enum Badge { ActiveWork { reason: String }, Reproducible { lockfile: String }, NoLockfile }
pub struct RecItem { /* v1 fields */ pub badges: Vec<Badge> }
// Items demoted by git_active form their own review-tier row, "<label> (active work)".
pub enum ViewKind { Dir, File, Smaller { count: u32 }, Virtual { provider: String } } // Virtual: container internals (Phase 18)
pub struct Filter { pub min_size: Option<u64>, pub max_size: Option<u64>, pub tier: Option<Tier>,
                    pub category: Option<String>, pub idle_days: Option<u32> } // UI model only
pub struct LocalSnapshot { pub name: String, pub date: String } // date matches ^\d{4}-\d{2}-\d{2}-\d{6}$
pub struct SweepRule { pub id: String, pub rule_ids: Vec<String>, pub scope: String,
                       pub idle_days: u32, pub schedule: SweepSchedule }
```

The staging queue (PRD 13.4.10) is the existing selection in `selection.rs`, shown in one place. It is never written to disk.

Selections survive a new snapshot by matching `(rule_id, path)` from `RecItem`, never by item ID (PRD FR-BG-4).

## 4. Backend functions

| Command | Input | Output |
|---------|-------|--------|
| `check_permissions` | none | `{ full_disk_access: bool }` |
| `open_fda_settings` | none | none |
| `load_last_snapshot` | none | `Option<SnapshotMeta>` |
| `start_scan` | `on_event: FnMut(ScanEvent)` | none |
| `cancel_scan` | none | none |
| `get_view` | `{ node_path: string, mode: ViewMode }` | `ViewNode` (root plus two levels, pruned) |
| `get_recommendations` | none | `Vec<Recommendation>` |
| `get_item_details` | `{ row_id }` | `{ items, largest_subfolders }` |
| `build_cleanup_plan` | `{ snapshot_id, item_ids, permanent_review: bool }` | `CleanupPlan` |
| `export_script` | `{ plan_id }` | `String` |
| `execute_cleanup` | `{ plan_id, on_event: FnMut(CleanupEvent) }` | none (a plan runs once, only while its snapshot is current) |
| `reveal_in_finder` | `{ item_id or node_path }` | none (only paths inside the snapshot) |

**v2 changes**

| Command | Input | Output |
|---------|-------|--------|
| `get_view` | `{ node_path: string }` | `ViewNode` (root plus one level, at most 12 tiles plus a `Smaller` tile) |
| `get_category_totals` | none | `Vec<CategoryTotal>` (sidebar and the Overview capacity bar) |
| `license_status` | none | `LicenseStatus` (Phase 15) |
| `activate_license` | `{ token: string }` | `LicenseStatus` or an error (Phase 15) |
| `find_folders` | `{ query, under: Option<path>, limit }` | `Vec<{ path, size }>` from the snapshot (palette, Phase 13) |
| `list_local_snapshots` | none | `{ snapshots: Vec<LocalSnapshot>, purgeable: u64 }` (Phase 17) |
| `thin_snapshots` / `delete_snapshot` | `{ bytes }` / `{ date }` | result; `delete_snapshot` asks for admin each time (Phase 17) |
| `get_models` | none | models grouped by family (Phase 19) |

`export_script` is unchanged; the UI writes its output to the file the user picks in the save panel.

These are plain functions in `src/backend.rs` taking `&Arc<AppState>`. The UI calls them through `src/ui/source.rs`, off the UI thread (`cx.background_spawn`); scans and cleanups run on their own thread and report through the callback. Shared state lives in a `Mutex`/`RwLock` inside `AppState`.

## 5. Phases

Phases 0 to 3 were built on Tauri and React. Their steps are kept as history; Phase 3.5 replaced that stack with GPUI.

### Phase 0: Scaffold

1. Create the Tauri v2 app with the React + TypeScript + Vite template using pnpm. Set the product name to DiskLens, add a placeholder bundle identifier, set minimum macOS to 13, and make the window 1280×860 with a 1100×720 minimum.
2. Add Tailwind v4, Biome v2, Vitest, zustand, d3-hierarchy, fonts and lucide-react. Add scripts for `dev`, `build`, `lint`, `test`, `test:rust`.
3. Write `CLAUDE.md` with commands, conventions, links to the docs, and the safety rules from section 0.
4. Write `scripts/make-fixture.sh` to build `.sandbox/home` with small real files covering:
   - every rule's target paths, plus near misses (for example `.next` without `package.json`)
   - a symlink pointing outside the sandbox and a hard-linked file
   - a git repo with `.next`, and a worktree with uncommitted changes
   - an idle and an active project with `node_modules`
   - protected paths (`Library/Keychains`, `.ssh`)
5. Add `src/mocks/snapshot.mock.json` reproducing the mockup's numbers.

**Acceptance:** `pnpm tauri dev` opens the window, `pnpm lint`, `pnpm test` and `cargo test` all pass, and the fixture script is idempotent.

### Phase 1: Scanner

1. `walk.rs` uses `jwalk` with `follow_links(false)`.
   - Read `lstat` metadata for each entry. Record `st_dev` of the root and skip directories on another device.
   - Skip dataless entries (`st_flags & SF_DATALESS`, value `0x40000000`).
   - Size is `st_blocks * 512`.
   - Record permission errors as inaccessible.
2. `hardlinks.rs`: for files with `st_nlink > 1`, record `(dev, ino) → (smallest path, size)`, then credit each inode once after the walk.
3. `tree.rs`: arena `Vec<DirNode>` with path interning, bottom-up aggregation, top 20 files per directory.
4. `volume.rs` reads capacity through `NSURL` resource values, falling back to `statfs`.
5. Progress is throttled to 100 ms and reported through a callback. Cancellation uses an `AtomicBool` checked inside the walk.
6. `snapshot.rs` saves and loads zstd-compressed JSON in Application Support.
7. Add the `start_scan`, `cancel_scan` and `load_last_snapshot` commands.
8. Add a `scan-bench` binary that scans a path, prints duration, files, directories and size, and compares the result with `du -sk`.

**Acceptance:**
- Fixture tests assert exact sizes, a hard link counted once, a symlink not followed, and inaccessible folders counted.
- A unit test covers the dataless flag check.
- `scan-bench` on `~/Library/Caches` and two project folders is within 1%. This is read-only and safe.
- Full home scan time is recorded against goal G1.

### Phase 2: Explorer UI

1. App shell matching PRD section 9: color tokens in `styles.css`, Geist fonts, card layout.
2. `StatCards` driven by store selectors. Selector math gets Vitest tests.
3. Backend `get_view`: root plus two levels, the 0.5% / top-24 "Other" pruning, and category and tier per node.
4. `Treemap`:
   - `d3-hierarchy` squarify with `paddingTop` for group headers and `paddingInner` 6.
   - Positioned divs with label thresholds and size font scaling.
   - Hatch overlay with `repeating-linear-gradient`, plus Category/Cleanup coloring.
   - Hover tooltip, click to drill, breadcrumb, Esc to go up, and a `ResizeObserver` relayout.
5. `ScanProgress` screen, plus empty and error states.
6. Run first against `snapshot.mock.json`, then against real scans of the sandbox.

**Acceptance:**
- With mock data, a screenshot is compared side by side with the mockup and differences are listed.
- Drill-down and breadcrumb work on a real scan.
- Treemap helpers have unit tests. (Since Phase 3.5: `src/ui/treemap.rs`, and the mock runs with `DISKLENS_MOCK=preselect cargo run`.)
- No layout work runs on hover.

### Phase 3: Rules engine and recommendations

1. Embed `rules.toml` with `include_str!`. Parse it with `deny_unknown_fields` and validate it as described in RULES.md section 3.
2. `category.rs`: nearest-ancestor resolution plus the media fallback.
3. `matcher.rs`: a compiled `GlobSet` per rule, the RULES.md section 4.1 algorithm, and the protect check.
4. `guards.rs`: match-time, blocking and safety guards. Process and app checks use `sysinfo` and `NSWorkspace.runningApplications`.
5. `tools.rs`: at startup, read `PATH` from `/bin/zsh -lc 'printf %s "$PATH"'` (with a timeout), merge it with known directories (`/opt/homebrew/bin`, `/usr/local/bin`, `~/.bun/bin`, `~/.local/bin`, `~/Library/pnpm`), and resolve tools to absolute paths.
6. Providers: `ollama`, `git_worktrees`, `simctl_unavailable`, `docker`.
7. `recommend/build.rs`: rows, row splitting, deduplication, totals, and sorting by size within each tier.
8. UI: the Recommendations panel per PRD section 7.4, "Select safe", the detail drawer, blocked states, and tree hatching driven by matches.

**Acceptance:**
- `tests/rules.rs` is table-driven, covering a match and a non-match for every rule ID plus overlap cases (for example Yarn versus `app-caches`).
- Validation tests reject malformed TOML.
- The sandbox produces the expected rows and totals.
- A test asserts the hardcoded and TOML protect lists are equal.

### Phase 3.5: GPUI migration

Replaces the Tauri shell and the React frontend with GPUI, before Phase 4 so the cleanup UI is built once.

1. Spike: GPUI window with Geist, a hatched tile, tooltip and context menu; release binary 5.2 MB (arm64). Done.
2. Docs, then restructure into one crate at the repo root (`src-tauri/` removed, React kept in `legacy-web/` as a reference until the end).
3. `commands.rs` becomes `backend.rs` (plain functions, scan progress through a callback).
4. GPUI skeleton with frozen module interfaces in `src/ui/`, then the pure logic (format, stats, selection, treemap, theme, model) and the views (screens, stat cards, explorer, recommendations) are ported from the React app.
5. Delete `legacy-web/`.

**Acceptance:**
- The Vitest suite is ported to Rust unit tests, and `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.
- The mock (`DISKLENS_MOCK=preselect`) is compared side by side with the mockup, and differences are listed.
- Phase 2 and 3 behaviour works on a sandbox scan in the native window.

### Phase 4: Cleanup engine

1. `protected.rs`: the hardcoded list and checks from RULES.md section 5.
2. `plan.rs`: item IDs to `CleanupPlan`. Plans are stored server-side by `plan_id` and invalidated by any rescan.
3. `revalidate.rs`: PRD S-03 checks and guard recheck.
4. `execute.rs`:
   - Sequential execution.
   - Delete with `std::fs::remove_dir_all` on the revalidated canonical path; `contents` iterates children.
   - Trash with the `trash` crate using `NsFileManager`.
   - Commands use `std::process::Command` with absolute binary, argv, a minimal environment and a 10 minute timeout.
   - Events stream over the channel and execution continues on error.
   - Dry-run support.
5. `script.rs`: script generation per PRD section 7.7 with `shell-escape`.
6. `audit.rs`: JSON lines log.
7. UI:
   - "Clean selected" opens `ConfirmDialog`, then `Progress`, then `Results`.
   - After cleanup, rescan the affected parent folders and refresh recommendations.
   - Add Copy script with a toast.
8. Adversarial test suite in `tests/cleanup_adversarial.rs`. Each case must be refused or skipped:
   - a symlink swapped in after the scan, and a path replaced by a symlink to outside home
   - a protected path injected into a plan, an unknown item ID, and an item ID from a previous snapshot
   - a repository root
   - a folder that grew past the threshold
   - a process guard that becomes active between plan and execute

**Acceptance:**
- All adversarial tests pass.
- Dry-run audit entries match the exported script line by line in meaning.
- Full cleanup works on the sandbox.
- One supervised real run deletes only the npm cache, and its size matches the freed space within 1%.

### Phase 5: Permissions, persistence, polish

1. FDA detection per PRD section 7.8, rechecked on window focus. Onboarding screen with the deep link.
2. On launch, show the last snapshot with "Scanned X ago" and Rescan.
3. Inaccessible-folders banner.
4. Dark mode for all tokens.
5. Keyboard navigation for tiles and rows, VoiceOver labels and focus rings.
6. Performance pass with a synthetic 2M-file tree. Measure peak memory and fix hotspots.

**Acceptance:** PRD section 10 targets are met and recorded, and onboarding works on a fresh macOS user account.

### Phase 6: Packaging and release

1. App icon, final bundle identifier, version 1.0.0.
2. Hardened runtime, no sandbox, Developer ID signing (`codesign`) and notarization (`notarytool`).
3. Universal build (`--target universal-apple-darwin`) and DMG.
4. Signed updates published to GitHub Releases (tool chosen in this phase, see Status).

**Acceptance:** PRD section 12 release criteria are all met.

### v2 phases

Rules for every v2 phase:
- Design work uses the `apple-design` skill and is checked against the HIG pages listed in PRD section 13.
- The engine (`scanner/`, `rules/`, `cleanup/`, `providers/`) changes only where a phase says so. Safety tests stay green and are never loosened.
- The mock source keeps working: `DISKLENS_MOCK=1` must show every new screen, so screenshots can be compared without a scan.
- Use a gpui-kit component where one fits and can be themed to the HIG; hand-build the rest.
- Every new rule gets a match test and a non-match test, and a fixture case in `scripts/make-fixture.sh`. Paths marked "verify" in PRD 13.10 are checked against the installed tool first.
- Pro phases add their rules with `pro = true` and test them with and without a license.

### Phase 7: Background rescan

Logic first, on the current UI, because it is independent of the redesign.

1. `AppModel`: split the scan state out of `Phase`. `Phase::Scanning` is used only when no snapshot exists yet; otherwise `scan: Option<ScanProgress>` is set while `Phase::Ready` stays.
2. Carry state over when a snapshot swaps in (PRD FR-BG-4): explorer path to the nearest existing ancestor, selection by `(rule_id, path)` with a toast for dropped items, drawer closed when its row is gone.
3. Disable Clean selected during a scan with its reason (FR-BG-5). Copy script, selection and browsing keep working.
4. Auto-rescan on launch when the snapshot is older than 24 hours (FR-BG-3).
5. After a cleanup, drop finished items from the recommendations and subtract their sizes from the view before the rescan (FR-BG-6).
6. Failed or cancelled background scans keep the results and show Retry next to Rescan (FR-BG-7).
7. Header status in the current Explorer card: "Updating… N files" with Cancel, or "Scanned X ago" with Rescan. Phase 8 moves it to the toolbar.

**Acceptance:**
- `TestAppContext` tests: a rescan never leaves `Phase::Ready`; the selection survives a swap by path; a vanished item is dropped and counted; the explorer path falls back to an ancestor; cleanup is refused while scanning; a stale snapshot triggers a scan on load and a fresh one does not.
- Manual check on the sandbox: rescan while browsing and while items are selected.

### Phase 8: Design foundation and window shell

1. Bump GPUI from `=0.3.9` to `=0.3.10` (`gpui-pre` and `gpui-pre-platform`, with the crates they pull in) in its own commit, with the full gate and a smoke run. Then add `gpui-component` (gpui-kit 0.8.1). Check that AccessKit labels still reach VoiceOver, that its theme can be driven entirely from `theme.rs`, and record which components are used and which are hand-built.
2. Produce `docs/mockup-v2.png` (light and dark: Overview, Explore with textures, Recommended, queue, command palette, cleanup sheet, onboarding) from the direction in PRD section 13.5, and get it approved before code. Update `assets/mock/snapshot.mock.json` to its numbers.
3. `theme.rs` v2 tokens: system font, tabular figures, the type scale, radii, spacing, system accent and status colors with AA-checked text shades, and the new category palette in `rules.toml` and `docs/RULES.md` section 7 (same commit, `tests/rules_doc_sync.rs`).
4. Check that GPUI resolves the system monospaced font. If not, keep Geist Mono for paths and record it. Remove Geist and the unused font files.
5. Window: transparent titlebar, traffic lights inset into the sidebar, `WindowBackgroundAppearance::Blurred`, new default and minimum sizes. All platform chrome differences go in one module (`src/ui/chrome.rs`).
6. Shell: sidebar (pages, categories with `get_category_totals`, footer notice), unified toolbar (title or breadcrumb, scan status from Phase 7, filter token slot, inspector toggle, queue button, Clean up button) and an empty inspector slot. Existing views move into pages unchanged for now.
7. Menu bar with the commands and shortcuts in PRD FR-WIN-4 (Command Palette is a disabled item until Phase 13).
8. Motion helpers: the two spring presets and the Reduce Motion and Reduce Transparency checks through `platform/`.
9. Line icon set in `assets/icons/ui/`.

**Acceptance:**
- Unit tests: token contrast (AA for every text-on-fill pair in both themes), category totals, the Reduce Motion fallback.
- The shell with mock data compared side by side with `docs/mockup-v2.png` in light and dark; differences listed.
- Keyboard: Tab reaches the sidebar, toolbar and content; the shortcuts work.

### Phase 9: Overview

1. Overview page with the four widgets in PRD FR-OV-1, built from `compute_stats`, `get_category_totals` and the root view.
2. Remove `stat_cards.rs` and the v1 "Not scanned" and "Free after cleanup" cards.
3. Largest folders open Explore at that folder.
4. Disk widget: the "Held by snapshots" line from the purgeable total (PRD FR-SNAP-3, free); listing the snapshots is Phase 17.

**Acceptance:** `stats.rs` tests updated for the new numbers (free now, after-cleanup delta, safe and review totals, purgeable); the mock compared with the mockup; VoiceOver reads each widget as one sentence.

### Phase 10: Explore and inspector

1. `view.rs`: one level, at most 12 tiles plus `Smaller`, 1% threshold, `cleanable` and `system_managed` per node (PRD FR-EXP-1, FR-EXP-2, FR-EXP-6). Remove `ViewMode`.
2. `treemap.rs`: label rules (both lines or nothing, 90×44 pt threshold, middle truncation) and spatial neighbour lookup (nearest tile centre per direction) as pure functions with tests.
3. `explorer.rs`: flat tiles with the semantic textures (PRD 13.5, Texture), cleanable line, lock glyph, texture key, Highlight cleanable, hover tooltip, context menu.
4. `keymap.rs`: the Explore keymap in PRD FR-KB-5 (h/j/k/l and arrows, Space, Return, Backspace/Esc/⌘↑, ⌘Y, `/` reserved for Phase 13). Quick Look through `QLPreviewPanel` in `platform/`.
5. Zoom transition on drill-down and its reverse, from and to the tile's frame (cross-fade with Reduce Motion).
6. Inspector panel (PRD 13.4.7) for folders, files and the smaller-items list.

**Acceptance:**
- `tests/view.rs`: 12-tile cap, 1% merge, `Smaller` children, `cleanable` sums, `system_managed` for protected paths and `~/Library` folders, and paths outside the snapshot still refused.
- `treemap.rs` tests: no label is ever laid out wider or taller than its tile, for random trees at the minimum window size; neighbour lookup picks the expected tile in each direction and never leaves the view.
- Theme test: label text passes AA over the darkest stripe pixel of every category, in both themes.
- `TestAppContext`: Space on a tile with no recommendation stages nothing; on a folder it stages exactly its ready items; Backspace at the root does nothing.
- Manual: the real home scan shows no clipped or overlapping text at the minimum window size (PRD V2-G2), with screenshots in `docs/QA.md`.

### Phase 11: Recommended, staging queue and cleanup sheet

1. Recommended page (PRD 13.4.6) with the scroll fade, the footer summary, badges, and j/k/Space/Return.
2. Row selection fills the inspector with the v1 drawer content; remove the drawer.
3. Staging queue (PRD 13.4.10) in the inspector from the toolbar queue button: grouped by rule, remove per item, Clear Queue, Copy as Shell Script, Export Cleanup Script…, Clean up.
4. Export Cleanup Script… through the save panel; Copy to Clipboard in the context menu.
5. Cleanup confirm, progress and results as one sheet from the toolbar, with the v1 behavior and the Enter rule unchanged. The results show the "held by local snapshots" line when free space grew less than the bytes removed (PRD 13.4.8).

**Acceptance:** the existing cleanup UI tests pass on the sheet; a test proves Enter does not confirm; Copy as Shell Script and the exported file both match `export_script` byte for byte; the queue is empty after a relaunch; the queue survives a snapshot swap by `(rule_id, path)`.

### Phase 12: Onboarding

1. The three-step flow in PRD 13.4.1 with a page indicator.
2. Spike first, on macOS 13, 14 and 15 with a fresh user account: which silent protected read (`~/.Trash` first) adds DiskLens to the Full Disk Access list without any prompt, and whether the list accepts a dragged `.app` URL. Record the results in `docs/QA.md`; anything that shows a prompt is dropped.
3. Open System Settings: the pre-seed read, the deep link, and moving the window to the right half of its screen (restored when the step ends). Window frame changes go in `chrome.rs`.
4. Drag card: toggle animation (still image with Reduce Motion) and a draggable app icon. The drag source is native (`NSDraggingSource` with the bundle's file URL, in `platform/`), because GPUI drags stay inside the window. Fallback if that fails: "Show DiskLens in Finder".
5. A 500 ms background timer on the FDA probe while the step is visible, plus the activation recheck; it stops when the step is left.
6. The first-scan view with categories filling in.
7. `onboarding_done` is written when the first scan finishes.

**Acceptance:** `TestAppContext` tests for each step, for an interrupted first run starting again at Welcome, for the poll advancing the step without window activation, and for polling stopping when the step is left; a test that onboarding code never touches `~/Desktop`, `~/Documents` or `~/Downloads` (a fake `System` that records probes); the fresh-account check in `docs/QA.md` repeated for v2: no TCC prompt appears, DiskLens is in the list after Open System Settings, and granting access advances within a second.

### Phase 13: Command palette

1. `palette.rs` on the gpui-kit input and list: ⌘K anywhere outside onboarding and sheets, ↑/↓ and ⌃N/⌃P, Return, Esc (PRD FR-KB-1).
2. Sources: pages, `find_folders` from the backend (snapshot search, ranked by `nucleo-matcher`), recommendation rows, menu commands with their shortcuts (FR-KB-2).
3. `>` command mode and the `filter` grammar as a pure parser with tests; the filter token in the toolbar; dimming in Explore and hiding in Recommended (FR-KB-3).
4. `/` in Explore opens the palette limited to the current folder.
5. Single-item stage and unstage commands. The Pro batch commands are listed with the Pro tag and open the explanation sheet until Phase 15.
6. Help › Keyboard Shortcuts.

**Acceptance:** parser tests for every filter term and for bad input; `TestAppContext`: ⌘K opens and Esc closes, a folder result drills Explore there, no palette command starts a cleanup (S-11); the palette opens in under 100 ms on the real home snapshot.

### Phase 14: Active-work guard and project artifacts

Engine change. Free.

1. `rules/git_state.rs` with `git2`: uncommitted and untracked changes, and commits on no remote branch (ahead of upstream, or no upstream and not on any remote). Errors count as active.
2. A new guard class, *demoting*, and the `git_active` guard: RULES.md section 4.2, `validate`, `matcher` and `recommend/build.rs` (demoted items form a review row "<label> (active work)" with the `ActiveWork` badge). Blocking and safety guards are unchanged.
3. New rules (PRD FR-GIT-4): Rust `target`, Python `.venv`/`venv`, `dist`; `git_active` added to `node-modules-idle` and `next-turbo-build`; `project_idle_days` evidence extended per marker (`Cargo.lock`, `pyproject.toml`, `uv.lock`). The new `keywords` field in RULES.md section 3, filled for every rule. `rules.toml` and RULES.md section 7 in the same commit.
4. Fixture: a dirty repo, a repo with unpushed commits, a branch with no upstream, a clean pushed repo, each with a `target` and `node_modules`.
5. Measure the bundle size with libgit2.

**Acceptance:** for each fixture repo, the right tier and badge; a corrupt repo demotes; match and non-match tests for each new rule; the git check over the sandbox stays under 50 ms per repository; DMG still under 25 MB.

### Phase 15: Pro licensing and gating

Starts only after the open decisions in PRD section 13.8 (price, provider, trial) are made.

1. `src/license.rs`: an offline-verified signed token (a public key in `assets/`, like `update.pub`), stored in the data dir. `license_status` and `activate_license` in `backend.rs`.
2. Pro tags on the Pro entry points and the one explanation sheet with purchase and license entry (PRD 13.6 gate rules).
3. The `pro = true` rule field: the matcher skips those rules without a license. The "More with Pro" entry at the end of Recommended.
4. Palette batch commands and saved filters (PRD FR-KB-4), all of which only stage.
5. A Settings window (⌘,) with the license, and the switches later Pro phases add (admin actions, agent, alerts).

**Acceptance:**
- Tests: a valid token activates Pro; a tampered, expired or wrong-key token does not; no network call is made.
- A test proves that scanning, recommendations of free rules, guards, plan building, execution and script export behave the same with and without a license.
- A test that a `pro = true` rule produces no rows without a license and does with one.

### Phase 16: Lockfile tags and orphaned stores

1. Lockfile detection next to the project manifest and the `Reproducible` and `NoLockfile` badges (PRD FR-LOCK-1).
2. Providers in `providers/store_orphans.rs`: pnpm (link count 1, from the scan's hard-link table), Bun (text `bun.lock` references; `bun.lockb` only means everything is used), Cargo (`Cargo.lock` references), Homebrew (`brew cleanup --dry-run`) (PRD FR-STORE-1). Review tier.
3. Fixture: a pnpm-style store with linked and unlinked files, a Bun cache with one referenced and one unreferenced package, a Cargo registry with one orphan crate.

**Acceptance:** orphan sizes in the sandbox match the fixture exactly; a package referenced from any lockfile under the root is never an orphan; the v1 `pnpm-store` row stays free and unchanged without a license.

### Phase 17: APFS snapshot inspector

1. Spike: whether `tmutil thinlocalsnapshots` needs admin on macOS 13 to 15, and the admin mechanism (PRD section 13.8). Record the results and the decision.
2. `snapshots.rs` behind `platform/`: `list_local_snapshots` (parsing `tmutil` output, fake-tested) and the purgeable total.
3. Inspector page: the list, the purgeable line, Thin, and Delete per snapshot (PRD FR-SNAP-1 to FR-SNAP-4).
4. Privileged path (PRD FR-SNAP-5): the Settings opt-in, the explanation sheet with the exact command, the date regex, re-reading the list right before acting, dry run, the audit method `privileged_command`.

**Acceptance:** tests: a date not in the current list is refused; a malformed date is refused; non-Time Machine snapshots are never offered (S-12); with the opt-in off, no privileged path runs; dry run never invokes the runner. Manual on macOS: thin and delete one snapshot, with free space before and after recorded in `docs/QA.md`.

### Phase 18: Containers and VMs

1. Disk image discovery for Docker Desktop, OrbStack, Colima and Lima (verify paths) (PRD FR-VM-1).
2. A minimal Docker API client over `UnixStream` (`GET /system/df`, plus the prune and delete endpoints), behind a trait with a fake (PRD FR-VM-2).
3. `ViewKind::Virtual` children for the image tile and their inspector view (PRD FR-VM-3).
4. Items and actions, with volumes per item and review tier, and the "shrinks later" note in the results (PRD FR-VM-4 to FR-VM-6).

**Acceptance:** tests against recorded `/system/df` JSON for each engine; a volume is never in Select safe; with the engine down the image is one tile; the v1 `docker-unused` rule is unchanged without a license.

### Phase 19: AI models hub

1. `providers/models.rs`: Ollama manifests (reusing the v1 provider), the Hugging Face hub layout, LM Studio folders; family, parameters and quantization from manifests, `config.json` and file names (PRD FR-AI-1, FR-AI-2).
2. Last used from Ollama server logs, then atime (PRD FR-AI-3).
3. Models page and actions (PRD FR-AI-4).

**Acceptance:** parser tests over a fixture of each layout; no test or code path opens a weight file; deleting a Hugging Face model removes the whole repo folder, never one snapshot.

### Phase 20: Apple, mobile and creative scanners

1. Simulator runtimes, DerivedData per project with the missing-workspace check, and old Gradle wrapper distributions (PRD FR-DEV-1).
2. Final Cut Pro, DaVinci Resolve, Adobe media cache and Figma rules with their app guards (PRD FR-CRE-1), each path verified first.
3. Fixture folders for each.

**Acceptance:** match and non-match tests for every rule; `High Quality Media` and `Proxy Media` are review tier; nothing outside the listed cache folders inside an `.fcpbundle` ever matches.

### Phase 21: Menu bar agent

1. Spike: whether the agent needs its own TCC grant for FSEvents on the scan root, and whether `SMAppService` login items work with ad-hoc signing. Decide where sweep scans run (the main binary in a headless mode, or the agent).
2. `src/bin/disklens-agent.rs`: AppKit status item through `objc2`, no GPUI; `SMAppService` registration from Settings; packaging into the bundle in `scripts/package.sh` (PRD FR-AGT-1, FR-AGT-2).
3. Low-disk alerts, and write-burst detection from free-space samples, `proc_pid_rusage` and FSEvents (PRD FR-AGT-3, FR-AGT-4). The burst detector is a pure function over samples, with tests.
4. Notification permission asked when the first alert is turned on.

**Acceptance:** burst detector tests (threshold, window, top writer); the agent never links the cleanup engine (checked by a test on its dependencies, S-11); resident memory under 15 MB after 24 hours, measured with `footprint` and recorded in `docs/QA.md`.

### Phase 22: Auto-sweep

1. `sweep.rs`: `SweepRule` storage in `settings.json`, validation (only existing rule IDs, scope under the scan root, idle days at least the rule's own) (PRD FR-SWP-1, FR-SWP-2).
2. Schedules in the agent; a run rescans the scope, matches, and writes the result for the main app (PRD FR-SWP-3, FR-SWP-4).
3. Notification, then opening DiskLens with exactly the sweep's items in the queue and the cleanup sheet open.
4. Sweep rule editor in Settings, and "Run sweep rule now" in the palette.

**Acceptance:** tests: a sweep never removes anything without the sheet's confirm (S-11); demoted (`git_active`) items are never swept; an invalid rule is refused; the example rules from PRD FR-SWP-1 stage the expected sandbox items.

### Phase 23 onward: remaining Pro features

One phase per feature (other volumes, history, large and old files, duplicates, local rules file, clone-aware sizes), in the order chosen in PRD section 13.8. Each needs its own PRD subsection in 13.10 before work starts.

## 6. Testing strategy

- **Rust unit tests** sit next to each module. Integration tests in `tests/` run against temp directories built from the fixture script logic.
- **UI:** unit tests for stats, selection, formatting, treemap layout and theme, plus `AppModel` transitions on GPUI's `TestAppContext` with the mock source. Views stay thin, so most logic is testable without rendering.
- **Manual QA checklist** lives in `docs/QA.md`, created in Phase 4: scan, drill-down, select, export, dry run, real run on the sandbox, and relaunch.
- CI is not required for v1. Every phase ends with `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` passing locally.

## 7. Definition of done (every task)

- Code is formatted and compiles with no warnings (`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`).
- Tests for the new behavior exist and pass.
- No TODOs left in safety-related code (`cleanup/`, `rules/guards.rs`, `rules/matcher.rs`).
- Docs are updated if behavior or rules changed.
