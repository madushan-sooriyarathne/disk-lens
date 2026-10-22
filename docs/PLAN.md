# DiskLens: Implementation Plan

Version 1.0 · Companion to `docs/PRD.md` and `docs/RULES.md`

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
| Shell | Tauri v2, macOS only, minimum macOS 13 |
| Backend | Rust stable, edition 2024 |
| Frontend | React 19, TypeScript (strict), Vite |
| Styling | Tailwind CSS v4 via `@tailwindcss/vite`; CSS tokens for colors and dark mode |
| State | zustand |
| Treemap | `d3-hierarchy` (`treemapSquarify`), rendered as positioned divs |
| Fonts | `@fontsource-variable/geist`, `@fontsource-variable/geist-mono` |
| Icons | `lucide-react` |
| Tooling | pnpm, Biome v2, Vitest |
| Type bridge | `ts-rs` generating TS types into `src/lib/bindings/` |

**Rust crates**

| Purpose | Crates |
|---------|--------|
| App shell | `tauri` |
| Serialization | `serde`, `serde_json`, `toml`, `zstd` |
| Rule matching | `globset` |
| Scanning | `jwalk`, `rayon`, `libc` |
| Processes | `sysinfo` |
| Trash | `trash` (macOS `DeleteMethod::NsFileManager`) |
| macOS APIs | `objc2`, `objc2-foundation`, `objc2-app-kit` (running apps, volume capacity) |
| Commands | `wait-timeout`, `shell-escape` (script export) |
| Logging | `tracing`, `tracing-appender` |
| Errors | `thiserror`, `anyhow` |
| Tests | `tempfile` |

Git operations shell out to the `git` binary.

## 2. Repository layout

```
disklens/
  docs/                 PRD.md, RULES.md, PLAN.md
  CLAUDE.md
  scripts/
    make-fixture.sh     builds .sandbox/home with realistic and adversarial cases
  src-tauri/
    rules/rules.toml
    src/
      main.rs, lib.rs, commands.rs, state.rs, error.rs
      scanner/  walk.rs, tree.rs, hardlinks.rs, volume.rs, snapshot.rs
      rules/    model.rs, load.rs, validate.rs, category.rs, matcher.rs, guards.rs, tools.rs
      providers/ ollama.rs, git_worktrees.rs, simctl.rs, docker.rs
      recommend/ build.rs
      cleanup/  protected.rs, plan.rs, revalidate.rs, execute.rs, script.rs, audit.rs
      platform/ fda.rs, apps.rs, finder.rs
    tests/      scanner.rs, rules.rs, cleanup_adversarial.rs, script.rs
    src/bin/scan-bench.rs
  src/
    main.tsx, App.tsx, styles.css
    lib/bindings/      generated types
    lib/api.ts         typed wrappers over invoke and channels
    lib/format.ts      bytes, relative time
    lib/treemap.ts     layout helpers
    store/             scan.ts, selection.ts, view.ts
    components/
      StatCards.tsx, Explorer/{Explorer,Treemap,Tile,Breadcrumb,Legend,Tooltip}.tsx
      Recommendations/{Panel,Row,Footer,DetailDrawer}.tsx
      Cleanup/{ConfirmDialog,Progress,Results}.tsx
      Onboarding.tsx, ScanProgress.tsx, Banner.tsx
    mocks/snapshot.mock.json   matches the mockup numbers
```

## 3. Data contracts

Rust types derive `Serialize` and `ts_rs::TS`. TypeScript types are generated, never hand-written.

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
    pub plan_id: String, pub actions: Vec<PlannedAction>, pub total: u64,
    pub trash_total: u64, pub warnings: Vec<String>,
}

pub enum ScanEvent { Progress { files: u64, bytes: u64, current: String }, Finished { meta: SnapshotMeta }, Failed { message: String } }
pub enum CleanupEvent { ItemStarted { item_id: String }, ItemFinished { item_id: String, freed: u64, outcome: Outcome }, Finished { summary: CleanupSummary } }
```

Item IDs are `"{rule_id}:{node_id}"` (or `"{rule_id}:{provider_key}"` for providers). They are valid only for the snapshot that produced them.

## 4. Tauri commands

| Command | Input | Output |
|---------|-------|--------|
| `check_permissions` | none | `{ full_disk_access: bool }` |
| `open_fda_settings` | none | none |
| `load_last_snapshot` | none | `Option<SnapshotMeta>` |
| `start_scan` | `Channel<ScanEvent>` | none |
| `cancel_scan` | none | none |
| `get_view` | `{ node_path: string, mode: ViewMode }` | `ViewNode` (root plus two levels, pruned) |
| `get_recommendations` | none | `Vec<Recommendation>` |
| `get_item_details` | `{ row_id }` | `{ items, largest_subfolders }` |
| `build_cleanup_plan` | `{ item_ids, permanent_review: bool }` | `CleanupPlan` |
| `export_script` | `{ plan_id }` | `String` |
| `execute_cleanup` | `{ plan_id, channel: Channel<CleanupEvent> }` | none |
| `reveal_in_finder` | `{ item_id or node_path }` | none (only paths inside the snapshot) |

Long work runs on blocking threads (`tauri::async_runtime::spawn_blocking`). Shared state lives in a `Mutex`/`RwLock` inside Tauri managed state.

## 5. Phases

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
5. Progress is throttled to 100 ms over a Tauri `Channel`. Cancellation uses an `AtomicBool` checked inside the walk.
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
- `lib/treemap.ts` helpers have Vitest tests.
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
2. Hardened runtime, no sandbox, Developer ID signing and notarization through Tauri's Apple environment variables.
3. Universal build (`--target universal-apple-darwin`) and DMG.
4. Tauri updater plugin with signed updates published to GitHub Releases.

**Acceptance:** PRD section 12 release criteria are all met.

## 6. Testing strategy

- **Rust unit tests** sit next to each module. Integration tests in `src-tauri/tests/` run against temp directories built from the fixture script logic.
- **Frontend:** Vitest for selectors, formatters and treemap helpers. Components stay thin, so most logic is testable without rendering.
- **Manual QA checklist** lives in `docs/QA.md`, created in Phase 4: scan, drill-down, select, export, dry run, real run on the sandbox, and relaunch.
- CI is not required for v1. Every phase ends with `pnpm lint && pnpm test && cargo test` passing locally.

## 7. Definition of done (every task)

- Code compiles with no warnings (`cargo clippy -- -D warnings`, `pnpm lint`).
- Tests for the new behavior exist and pass.
- No TODOs left in safety-related code (`cleanup/`, `rules/guards.rs`, `rules/matcher.rs`).
- Docs are updated if behavior or rules changed.
