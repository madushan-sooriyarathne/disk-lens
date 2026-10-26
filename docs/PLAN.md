# DiskLens: Implementation Plan

Version 1.0 · Companion to `docs/PRD.md` and `docs/RULES.md`

## Status and roadmap (as of 2026-10-25)

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

Last full gate run on macOS (Phase 6, 2026-10-25): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (313 tests) all pass.

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

**Housekeeping**
- `platform/` is one `mod.rs` with the `System`, `Runner` and `Trash` traits. Phase 5 adds FDA code there.

### After v1

PRD section 13 lists the v2 candidates: large and old files finder, duplicate finder, clone-aware sizes, menu bar indicator, scheduled safe-tier cleanup, and a user-editable rules file.

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
| UI | GPUI, Zed's GPU UI framework: `gpui-pre` and `gpui-pre-platform` pinned to an exact snapshot version (`=0.3.9`), imported as `gpui` and `gpui_platform`. No component library |
| Styling | Theme tokens in `src/ui/theme.rs`; category colors come from `rules.toml` |
| State | One GPUI entity, `AppModel` (`src/ui/model.rs`) |
| Treemap | Squarified layout ported from d3 into `src/ui/treemap.rs`, rendered as absolutely positioned divs |
| Fonts | Geist and Geist Mono static TTFs (OFL) embedded from `assets/fonts/` |
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

Git operations shell out to the `git` binary.

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
