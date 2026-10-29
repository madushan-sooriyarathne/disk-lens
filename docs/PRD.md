# DiskLens: Product Requirements

Version 2.0 draft · Owner: Madushan · Status: v1 built (sections 1 to 12); v2 defined in section 13, not started

Related documents: `docs/RULES.md` (classification and cleanup rules), `docs/PLAN.md` (implementation plan).

Sections 1 to 12 describe v1. Section 13 describes v2 and wins where the two differ; the v1 sections it replaces say so at their top.

## 1. Summary

DiskLens is a macOS desktop app for developers. It scans the home folder, shows space usage as an interactive treemap, explains in plain language what each large folder is, and reclaims space from caches, build output and other regenerable data only when the user selects it. Every cleanup can also be exported as a readable shell script.

## 2. Problem

Developer Macs quietly accumulate tens of gigabytes in package caches, build artifacts, mobile SDKs, emulator images, local AI models and stale git worktrees. macOS Storage settings hide most of this behind labels like "System Data" or "Developer" with no explanation. Generic cleaners either ignore developer folders or delete aggressively without explaining the consequences, so users end up running `du` by hand and guessing what is safe.

## 3. Goals

| ID | Goal | Measure |
|----|------|---------|
| G1 | Show where space goes, quickly | Full scan of a 150 GB home folder with about 1.5M files in under 60 s on Apple Silicon |
| G2 | Explain every large folder | Every node of 1 GB or more has a category and a one-line description |
| G3 | Safe by construction | Zero deletions outside rule-matched, revalidated paths, enforced by automated tests |
| G4 | Transparent | Every cleanup plan can be exported as a bash script equivalent to what the app runs |
| G5 | Accurate | Folder sizes within 1% of `du -sk` for the same path |

## 4. Non-goals for v1

- Scanning outside the home folder (reported as "Not scanned")
- Duplicate finder, app uninstaller, login item manager
- Scheduled or automatic cleanup
- Mac App Store distribution (the sandbox would block scanning)
- Windows or Linux
- Cleaning user documents, photos or downloads

## 5. Users

**Primary:** developers working with web, mobile and AI tooling on Apple Silicon Macs with 256 GB to 1 TB disks. They know what npm or Gradle is but don't want to remember where every tool hides its cache.

**Secondary:** technical power users who want a clear picture of their disk.

*v2 widens the audience (section 13).*

## 6. User stories

| ID | Story | Acceptance |
|----|-------|------------|
| US-01 | On first launch I see why Full Disk Access is needed and can open the right settings pane | Button deep-links to Privacy & Security; I can continue without granting; status rechecked when the window regains focus |
| US-02 | I watch a scan progress and can cancel it | Shows files counted, bytes, current folder; UI stays responsive; cancel stops within 1 s |
| US-03 | After the scan I see stat cards and a treemap of my home folder | Matches layout in section 9 |
| US-04 | I click a folder tile to drill into it and use the breadcrumb to go back | Esc goes up one level |
| US-05 | Hovering a tile tells me what it is | Tooltip shows full path, size, category and rule description when matched |
| US-06 | I switch between Category and Cleanup views | Category view colors by category; Cleanup view colors by tier |
| US-07 | I see recommendations grouped into "Safe to remove" and "Review first" | Sorted by size within each tier; nothing selected by default |
| US-08 | I open a recommendation to understand it | Drawer shows description, what happens after removal, items with per-item checkboxes, largest subfolders, guard status, Reveal in Finder |
| US-09 | Selecting items updates totals immediately | "Free after cleanup" and the footer total update on every change |
| US-10 | "Select safe" selects every ready safe-tier item | Blocked items stay unselected |
| US-11 | "Copy script" puts a bash script for my selection on the clipboard | Toast confirms; script matches section 7.7 |
| US-12 | "Clean selected" removes what I selected after I confirm | Confirmation, progress, results summary, then affected folders are rescanned |
| US-13 | On relaunch I see my last scan instantly | Shows "Scanned 2 h ago" with a Rescan button |

## 7. Functional requirements

### 7.1 Scanning

- **FR-SCAN-1** The scan root is the user's home folder. A development override `DISKLENS_SCAN_ROOT` points the app at a fixture sandbox.
- **FR-SCAN-2** Traversal is parallel. Symlinks are recorded but never followed, and the scan never crosses into another filesystem (compare `st_dev`).
- **FR-SCAN-3** Size is allocated size (`st_blocks × 512`). Logical size is recorded too. Files with more than one hard link count once globally and are attributed to the lexicographically smallest path seen for that inode, so results are deterministic.
- **FR-SCAN-4** Dataless files (iCloud placeholders with the `SF_DATALESS` flag) count as 0 bytes and are never opened, so the scan never triggers downloads.
- **FR-SCAN-5** Folders that can't be read are marked inaccessible. Their count drives a banner suggesting Full Disk Access.
- **FR-SCAN-6** The backend keeps a directory tree with aggregated sizes. Individual files are not kept, except the 20 largest files per directory, which appear as leaf tiles.
- **FR-SCAN-7** Volume capacity comes from `NSURLVolumeAvailableCapacityForImportantUsageKey` (matches Finder, includes purgeable space), with `statfs` as a fallback.
- **FR-SCAN-8** Progress events are sent at most every 100 ms.

### 7.2 Stat cards

*Replaced in v2 by the Overview page (section 13.4.4).*

| Card | Value | Secondary |
|------|-------|-----------|
| Free after cleanup | available space + selected total | green delta `+52.5 GB` equal to the selected total; hidden when 0 |
| Safe to remove | sum of ready safe-tier items | item count |
| Needs review | sum of ready review-tier items | item count |
| Not scanned | volume total − available − scanned home size, clamped at 0 | label "system & apps" |

Sizes marked as estimates (see RULES.md) show as `up to 4.1 GB`. Totals never double count: each path belongs to at most one recommendation.

### 7.3 Explorer (treemap)

*Replaced in v2 by section 13.4.5.*

- Shows the current root and two levels below it. Groups (first level) render a header with name on the left and size on the right. Their children render as tiles inside.
- Layout uses the squarified algorithm. Children smaller than 0.5% of their parent's area, or beyond the 24 largest, merge into a single "Other" tile.
- **Category view:** fill and border colors come from the category (section 9).
- **Cleanup view:** safe items are green, review items amber, everything else neutral gray.
- **In both views:** tiles matched by a recommendation get a diagonal hatch overlay in the tier color.
- Labels: the name sits top left. The size sits bottom left in mono, with font size scaling with tile area from 13 to 22 px. The name hides below 60×28 px and the size hides below 80×44 px.
- Clicking a directory tile drills into it, while clicking a file or "Other" tile does nothing. Right-click opens a menu with Reveal in Finder and Show recommendation when matched.
- The header shows the breadcrumb (`~ Home / Desktop / atlas`) and the current root size. A legend of the six categories sits below the treemap.
- Resizing the window relayouts without visible jank.

### 7.4 Recommendations panel

*Replaced in v2 by section 13.4.6. Rows, tiers, blocked states and Select safe keep their v1 behavior.*

- Title "Recommendations" with subtitle "Select items to build a cleanup script".
- Two sections, each with a tinted pill ("Safe to remove" green, "Review first" amber) and its tier total on the right.
- Each row shows a checkbox, label, a mono subtitle of paths (truncated with ellipsis, full list in a tooltip), size, and a chevron.
- Blocked rows show a disabled checkbox and a short reason such as "Quit Xcode to clean".
- The footer shows `52.5 GB selected · 5 items` with three buttons: "Select safe", "Copy script" (secondary) and "Clean selected" (primary, disabled when nothing is selected).

**Change from the mockup:** "Clean selected" is added as the primary action, and "Copy script" becomes secondary.

### 7.5 Detail drawer

*In v2 this content moves into the inspector (section 13.4.7).*

Opens from the chevron and slides over the recommendations panel. It contains:

- The label and tier pill
- "What it is" (rule `description`)
- "What happens if removed" (rule `regenerates`)
- The method in plain words: "Deleted permanently", "Moved to Trash" or "Runs `pnpm store prune`"
- An item list with per-item checkboxes, sizes, and status (ready or blocked with reason)
- The five largest subfolders of the selected items
- A "Reveal in Finder" button for each item

### 7.6 Cleanup execution

1. "Clean selected" asks the backend to build a cleanup plan from the selected item IDs.
2. The confirmation dialog lists actions grouped by rule, with sizes, method, warnings and the total.
3. For review-tier items whose method is trash, a checkbox "Delete permanently instead of moving to Trash" appears. It is off by default and hidden when the rule sets `allow_permanent = false`.
4. When some items will be trashed, the dialog explains that space is freed only when the Trash is emptied.
5. The confirm button reads `Clean 52.5 GB` and must be clicked. Enter does not trigger it.
6. Items execute sequentially with per-item progress. A failure skips that item and continues.
7. The results view shows freed space, space moved to Trash, and skipped items with reasons.
8. Affected parent folders are rescanned and the snapshot is updated.

### 7.7 Script export

- Generated by the backend from the same plan builder as execution.
- Starts with `#!/bin/bash` and `set -euo pipefail`, plus a header comment with app version, date, total and item count.
- Each step echoes what it is doing, then runs the equivalent command:
  - `rm -rf -- '<path>'` for delete
  - `find '<dir>' -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +` for content deletes
  - an `osascript` Finder delete for trash
  - the rule's command for command rules
- Paths are single-quote escaped. The script contains no variables derived from user input.

### 7.8 Permissions

- Full Disk Access is detected by attempting to open `~/Library/Application Support/com.apple.TCC/TCC.db` for reading. Permission denied means access is not granted.
- Onboarding explains why access is needed. The button opens `x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles`.
- The app never asks for an administrator password. *v2 makes one opt-in exception for snapshot actions (S-06, section 13.10.5).*

### 7.9 Persistence

- Snapshot: `~/Library/Application Support/DiskLens/snapshot.json.zst`
- Settings: `~/Library/Application Support/DiskLens/settings.json`
- Audit log: `~/Library/Logs/DiskLens/cleanup.log` (JSON lines)

## 8. Safety requirements

These are non-negotiable and each one has automated tests.

- **S-01** Only items produced by the rules engine for the current snapshot can be cleaned. The UI sends item IDs, never paths.
- **S-02** Hardcoded protected paths (RULES.md section 5) are refused in code regardless of `rules.toml`.
- **S-03** Each item is revalidated immediately before it runs. It is skipped as "changed since scan" if any of these fail:
  - the canonical path is inside home
  - the path is not a symlink and not a mount point
  - it still matches the same rule
  - it is not protected
  - it has not grown by more than 20% and more than 1 GB since the scan
- **S-04** Guards are evaluated when the plan is built and again before each item executes.
- **S-05** Commands run as argv arrays, never through a shell. Binaries resolve to absolute paths, the environment is minimal, and each command has a 10 minute timeout.
- **S-06** No elevated privileges are requested or used. *v2 exception:* the opt-in snapshot actions in section 13.10.5. Each one explains why before asking, shows the exact command, asks through the system administrator prompt for that one action, keeps no authorization afterwards, and is audit logged. Nothing else ever runs as root.
- **S-07** Every action is logged with timestamp, rule, path, method, bytes and result.
- **S-08** Dry-run mode (`DISKLENS_DRY_RUN=1`, default on in debug builds) logs actions without performing them.
- **S-09** The default selection is empty.
- **S-10** A directory containing a `.git` directory (a repository root) is never deleted by path.
- **S-11** (v2) Only the confirm button of the cleanup sheet starts a cleanup or a privileged action. Shortcuts, command palette commands, auto-sweep rules and the menu bar agent can stage items and notify. They never remove anything.
- **S-12** (v2) Snapshot actions touch only Time Machine local snapshots that `tmutil listlocalsnapshots` lists at the moment of the action. Other APFS snapshots (system and update snapshots) are never offered.

## 9. Visual specification

*Replaced in v2 by section 13.5. Kept as the reference for `docs/mockup.jpg` and the v1 mock.*

**Layout**
- The window is 1280×860 by default, with a minimum of 1100×720 and a light gray window background (`#F7F7F8`).
- Row 1 is one bordered container holding four stat cards separated by vertical dividers.
- Row 2 holds the Explorer card (flexible width) on the left and the Recommendations card (390 px) on the right, 16 px apart.
- Cards have a 12 px radius, a 1 px `#E6E6E9` border and a white background.

**Typography**
- Geist is used for UI text. Geist Mono is used for numbers, sizes and paths.
- Stat value: 30 px mono, semibold. Stat label: 13 px muted with a 6 px status dot. Delta: 13 px mono green.
- Row label: 15 px medium. Row path: 12 px mono muted. Row size: 15 px mono medium.

**Category colors**

| Category | Fill | Border |
|----------|------|--------|
| Code projects | `#E6E4FB` | `#8E84E8` |
| AI models | `#FBDDE6` | `#E58AA7` |
| iOS & Android SDKs | `#D9EEF4` | `#6FB7CC` |
| Package caches | `#DCE5F7` | `#7C9BD8` |
| Documents & media | `#FCE6D8` | `#EFA57A` |
| Dev tools & other | `#EEEEF0` | `#C4C4C9` |

**Tier colors**
- Safe: `#22A06B`. Hatch is 45° stripes, 2 px lines every 7 px at 35% opacity, and the border turns green.
- Review: `#E2A03F` with the same hatch pattern.

**Dark mode:** all colors are defined as CSS tokens with dark equivalents. Dark mode is required by the end of Phase 5.

## 10. Non-functional requirements

- **Performance:** meets G1. Treemap layout takes under 16 ms for 400 tiles. Peak memory stays under 400 MB for 2M files.
- **Bundle:** under 25 MB, universal binary (Apple Silicon and Intel), macOS 13 or later.
- **Accessibility:** keyboard navigation across tiles and rows, VoiceOver labels, and WCAG AA text contrast.

## 11. Risks and mitigations

| Risk | Mitigation |
|------|-----------|
| Scan triggers iCloud downloads | Dataless detection; file contents are never read |
| Repeated TCC prompts for Desktop, Documents, Downloads | Ask for Full Disk Access once during onboarding |
| APFS clones make sizes look larger than reclaimable space | Documented limitation for v1; v2 can use `ATTR_CMNEXT_PRIVATESIZE` |
| GUI apps don't inherit the shell `PATH`, so tools aren't found | Resolve tools through a login shell lookup plus known install directories |
| Command-based cleanups free an unknown amount | Mark as estimates and show "up to" |
| Deleting caches of running apps causes errors | Process and app guards, rechecked at execution |
| A path changes between scan and cleanup | Revalidation (S-03) |
| (v2) The FDA list doesn't show DiskLens until it has made a TCC check | One silent protected read before opening System Settings, plus the drag card (FR-ONB-3a) |
| (v2) GPUI can't drag a file to another app | The drag source is native (`NSDraggingSource` through `objc2` in `platform/`). If the spike fails, "Show DiskLens in Finder" lets the user drag from Finder |
| (v2) The ad-hoc signature changes with every build, so macOS forgets the FDA grant | Developer ID signing (Phase 6 open item). Until then, the release notes say so |
| (v2) gpui-kit pins a newer GPUI snapshot than DiskLens | Bump GPUI in its own commit first (Phase 8). Any component that can't follow the HIG is hand-built |
| (v2) Freeing space inside a VM doesn't shrink its disk image | Said in the results (FR-VM-5) |
| (v2) Access times on APFS are lazy | "Last used about …" wording, "Unknown" without a signal (FR-AI-3) |
| (v2) A separate agent binary may need its own TCC grant | Phase 21 spike: the agent hands scans to the main binary in a headless mode (same identity), or asks for access itself |
| (v2) libgit2 adds bundle size | Measured in Phase 14 against the 25 MB DMG target |

## 12. Release criteria for v1

- All safety tests pass, including the adversarial fixture suite.
- Scanner accuracy is within 1% of `du -sk` on three real folders.
- Every rule in RULES.md has a match test and a non-match test.
- The UI matches the mockup with mock data, judged side by side.
- The app is signed, notarized, and launches on a clean macOS 13 machine.

## 13. Version 2

v2 keeps the v1 engine (scanner, rules, safety, cleanup) and rebuilds the experience around it. Six themes:

1. A proper onboarding flow.
2. Rescans in the background, over the previous results, instead of a blocking scanning screen.
3. A complete UI overhaul that follows Apple's Human Interface Guidelines for macOS, so the app feels as if Apple built it.
4. A simpler treemap that an average user can read, with textures that show reclaimable space at a glance.
5. A keyboard-first workflow: a command palette (⌘K), Vim-style treemap navigation and a staging queue.
6. Deeper knowledge of the tools that fill workstations: git-aware project cleanup, AI models, containers and VMs, Apple and Android SDKs, video and design caches, and APFS local snapshots.

v2 also splits the product into a free tier and a Pro tier (section 13.6). Pro features are specified in section 13.10.

**Audience:** intermediate to power users on macOS: developers, video editors, designers and others whose tools leave large caches, build output and models behind. They value convenience over managing every folder by hand, have their workflows set up, and expect to drive the app from the keyboard. Section 5 still describes the developer core.

**Design references:** the HIG pages for [design principles](https://developer.apple.com/design/human-interface-guidelines/design-principles), [designing for macOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-macos), [foundations](https://developer.apple.com/design/human-interface-guidelines/foundations), [patterns](https://developer.apple.com/design/human-interface-guidelines/patterns), [components](https://developer.apple.com/design/human-interface-guidelines/components), [inputs](https://developer.apple.com/design/human-interface-guidelines/inputs) and [technologies](https://developer.apple.com/design/human-interface-guidelines/technologies). Every design task uses the `apple-design` skill. `docs/mockup-v2.png` (produced in Phase 8) becomes the visual target and replaces `docs/mockup.jpg` for v2 work.

### 13.1 Goals

| ID | Goal | Measure |
|----|------|---------|
| V2-G1 | Results are always on screen | After the first scan, no screen ever replaces the results while a scan runs |
| V2-G2 | Readable at a glance | Every visible treemap tile shows its full name or no text at all; no clipped or overlapping text at any window size down to the minimum |
| V2-G3 | Feels native | Side-by-side review against Finder and System Settings › Storage finds no off-platform spacing, type or control |
| V2-G4 | Understandable by a non-developer | In a hallway test, 4 of 5 people can say how much they can safely free within 10 seconds of the Overview appearing |
| V2-G5 | Fair free tier | Everything v1 does stays free; Pro only adds new capabilities |
| V2-G6 | Keyboard-complete | Every action in Overview, Explore, Recommended and the cleanup sheet works without the mouse; ⌘K opens the palette from any page |

### 13.2 Non-goals for v2

- Windows and Linux builds. GPUI stays the UI stack so they remain possible later, so v2 keeps platform-specific code behind `platform/` and the few UI chrome differences (section 13.5) in one place.
- Mac App Store distribution (unchanged reason: the sandbox blocks scanning).
- A native AppKit or SwiftUI rewrite.
- Changing safety rules S-01 to S-10. v2 adds rules (S-11, S-12); it never relaxes one, except the narrow S-06 exception for opt-in snapshot actions (decided 2026-10-29, section 13.10.5).
- Running in the background with root. The app and the menu bar agent stay in user space; admin rights are asked for one action at a time.

### 13.3 User stories

| ID | Story | Acceptance |
|----|-------|------------|
| US-20 | On first launch a short welcome tells me what DiskLens does and that nothing is deleted without my confirmation | Three short points, one Continue button, under 10 seconds to read |
| US-21 | Onboarding helps me grant Full Disk Access, and moves on by itself when I do | DiskLens is already in the Full Disk Access list or can be dragged into it from the onboarding card; granting access advances the step within a second, without switching back to DiskLens; no other permission prompt appears; "Continue without" is always available |
| US-22 | My first scan shows categories filling in as they are found | The first scan is the only one that uses a full-window progress view |
| US-23 | When I rescan, I keep working with the previous results | Toolbar shows progress and Cancel; results swap in place when the scan finishes |
| US-24 | When I relaunch, I see my last results at once and they refresh on their own when stale | Snapshots older than 24 hours start a background rescan on launch |
| US-25 | The Overview tells me how much I can free safely, in one number | "Safe to clean" widget with total, item count and Select safe |
| US-26 | The treemap shows one level at a time and every label is readable | Rules in section 13.4.5 |
| US-27 | I can tell which folders I can't do anything about | System-managed folders are gray with a lock |
| US-28 | Selecting a tile or a recommendation shows details beside it | Inspector on the trailing side, toggled from the toolbar |
| US-29 | After a cleanup, cleaned items leave the screen right away | Removed before the follow-up background rescan finishes |
| US-30 | Pro features are visible but never stand between me and cleaning what was found | Section 13.6 gate rules |
| US-31 | I press ⌘K and type where to go or what to do | Pages, folders, recommendations, commands and filters in one fuzzy list (section 13.4.9) |
| US-32 | I move around the treemap with h/j/k/l or the arrows, stage with Space, open with Return and go up with Backspace | Keymap in FR-KB-5 |
| US-33 | I collect items from any page in one staging queue and can copy it as a shell script before running it | Section 13.4.10 |
| US-34 | I can see which tiles are disposable space and which are my data without reading labels | Textures in section 13.5 |
| US-35 | A project with uncommitted or unpushed work is never offered as safe, however old it is | "Active work" badge, review tier (section 13.4.11) |
| US-36 | When a cleanup frees less than it removed, I am told why | Space held by local snapshots is named in the results and on the Overview (section 13.10.5) |

### 13.4 Functional requirements

#### 13.4.1 Onboarding

- **FR-ONB-1** Shown when `onboarding_done` is false. Three steps in one window-sized card with a page indicator: Welcome, Full Disk Access, First scan.
- **FR-ONB-2** Welcome: app icon, title, three short points (see what fills your disk; find space that is safe to free; nothing is removed without your confirmation, and most items go to the Trash). One Continue button.
- **FR-ONB-3** Full Disk Access detection stays the silent probe from section 7.8: opening `TCC.db` fails with `EPERM` when access is missing and never shows a prompt. Nothing in onboarding (or before it) reads `~/Desktop`, `~/Documents`, `~/Downloads` or any other folder with its own TCC prompt, because each of those prompts is redundant once Full Disk Access is granted.
- **FR-ONB-3a** The Full Disk Access step shows:
  - Why access is needed, in one sentence.
  - **Open System Settings.** Deep links to `x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles`. Right before opening it, the app makes one silent read of a TCC-protected path so that macOS lists DiskLens in the pane and the user doesn't have to click + and look for it in `/Applications`. `~/.Trash` is the first candidate. Phase 12 confirms on macOS 13, 14 and 15 which path adds the app to the list without showing a prompt.
  - **Window placement.** When System Settings opens, DiskLens moves to the right half of its screen so the card stays visible next to System Settings (which opens on the left or in the centre). The window returns to its previous frame when the step ends.
  - **Drag card.** A short looping animation of the toggle being switched on, and a draggable DiskLens icon with the line "Don't see DiskLens? Drag this icon into the list." The drag carries the `.app` bundle's file URL, which the Full Disk Access list accepts. With Reduce Motion the animation is a still image.
  - "Continue without", always available.
- **FR-ONB-3b** While the step is on screen, a background timer polls the probe every 500 ms. It also rechecks on window activation. Once access is granted the step shows a check mark and advances after a short pause, even while System Settings is in front. Polling stops when the step is left.
- **FR-ONB-7** Onboarding never asks for administrator rights. The app stays in user space. The only admin prompt in DiskLens is the opt-in snapshot action (section 13.10.5), which says that it needs admin rights before it asks.
- **FR-ONB-4** First scan: starts automatically. A full-window progress view shows file count, bytes and the six categories filling in as their totals grow. Cancel returns to the idle screen.
- **FR-ONB-5** `onboarding_done` is written when the first scan finishes, not when the user leaves the access step, so an interrupted first run starts again at the welcome step.
- **FR-ONB-6** Notification permission is never asked during onboarding. It is asked the first time a feature needs it (Pro alerts).

#### 13.4.2 Background rescan

- **FR-BG-1** After the first snapshot exists, `Phase::Ready` stays on screen during a scan. The scan state becomes a separate field of `AppModel` (`scan: Option<ScanProgress>`).
- **FR-BG-2** The toolbar shows the scan state: a small progress indicator with "Updating… 412,000 files" and a Cancel button while scanning; "Updated 3 min ago" and a Rescan button otherwise.
- **FR-BG-3** On launch, the last snapshot is shown at once. If it is older than 24 hours, a background rescan starts. The threshold is a constant, not a setting.
- **FR-BG-4** When a scan finishes, the new snapshot replaces the old one in one frame. The UI carries over by path, not by ID, because item IDs change with every snapshot:
  - the explorer path, falling back to the nearest ancestor that still exists
  - the selection, matched by `(rule_id, path)`; items with no match are dropped, and a toast says how many
  - the inspector subject, closed if it no longer exists
- **FR-BG-5** While a scan runs, Clean up is disabled with the reason "Available when the update finishes". Plans are tied to a snapshot (section 3 of PLAN.md), so this avoids building a plan that the swap would invalidate. Selecting, browsing, Copy script and Reveal in Finder keep working.
- **FR-BG-6** After a cleanup, items that were deleted or trashed are removed from the current view at once (sizes and totals updated in the UI model), then a background rescan of the whole root runs as in v1.
- **FR-BG-7** A failed or cancelled background scan keeps the current results and shows the error in the toolbar status with Retry.

#### 13.4.3 Window structure

- **FR-WIN-1** A sidebar and a content area under a unified toolbar. Sidebar items: Overview, Explore, Recommended; then a Categories section listing the six categories with color dots, sizes and counts (it doubles as the legend); then a footer with the Full Disk Access notice (only while access is missing) and the Pro status.
- **FR-WIN-2** Toolbar, left to right: sidebar toggle; back and forward (Explore only); the page title, or the breadcrumb on Explore; flexible space; scan status (FR-BG-2); active filter token (FR-KB-3); inspector toggle; queue button (FR-Q-3); the primary button `Clean up <staged total>`.
- **FR-WIN-3** The primary button stays in place. With nothing staged it reads "Clean up" and is disabled; the queue starts empty (S-09).
- **FR-WIN-4** The app has a full menu bar: File (Rescan ⌘R, Export Cleanup Script… ⌘E, Reveal in Finder ⌘⇧R), Edit (Add to Queue, Select Safe Items, Clear Queue), View (Command Palette ⌘K, Overview ⌘1, Explore ⌘2, Recommended ⌘3, Show Inspector ⌥⌘I, Show Queue, Highlight Cleanable ⌘⇧H, Quick Look ⌘Y, Show Sidebar ⌃⌘S), Window, Help (Keyboard Shortcuts).
- **FR-WIN-5** The unreadable-folders banner (FR-SCAN-5) becomes the sidebar footer notice. No full-width banners.

#### 13.4.4 Overview

- **FR-OV-1** Widgets in a grid, each with one number and one visual:
  - **Disk:** volume name, free space as the large number, and a segmented capacity bar (scanned categories, "System and apps" for the not-scanned remainder, free). With a selection, a "+52.5 GB after cleanup" line appears under the number.
  - **Safe to clean:** total and item count in green, the line "Caches and build output that rebuild on their own", and a Select safe button.
  - **Worth a look:** review-tier total and count in amber, and a Review button that opens Recommended.
  - **Largest folders:** the five largest folders directly under the scan root, each with a size bar; clicking one opens it in Explore.
- **FR-OV-2** The confusing v1 cards ("Free after cleanup" as a separate number, "Not scanned 0 B") are gone. Their data lives in the Disk widget.

#### 13.4.5 Explore (treemap)

- **FR-EXP-1** Shows one level: the direct children of the current folder. No group headers and no tiles inside tiles.
- **FR-EXP-2** At most 12 tiles. Children beyond the 12 largest, or under 1% of the current folder, merge into one tile labeled "N smaller items" with their total size. It is always last, drawn with a dashed outline, and opens a sorted list of those items in the inspector.
- **FR-EXP-3** Labels: name on line 1, size on line 2, both top left. Below 90×44 pt a tile shows no text; hovering shows a tooltip with name, size, category and description. Long names truncate in the middle.
- **FR-EXP-4** Tiles have 4 pt gaps, a 10 pt corner radius, no borders, and a fill from the category color with a semantic texture (section 13.5, Texture).
- **FR-EXP-5** A tile that contains recommended items shows a second line "9.1 GB cleanable" in place of the plain size line when it fits. A tile that is itself a recommended item is textured as disposable or restorable (section 13.5), which replaces the v1 hatch overlay.
- **FR-EXP-6** System-managed folders (protected paths from RULES.md section 5, and folders under `~/Library` with no recommendation anywhere inside) use the neutral fill with a lock glyph and the tooltip "Managed by macOS or an app".
- **FR-EXP-7** "Highlight cleanable" (toolbar menu and ⌘⇧H) dims every tile without cleanable content. It replaces the v1 Category/Cleanup mode toggle.
- **FR-EXP-8** Interaction: click selects a tile and fills the inspector; double-click or Return opens a folder with a zoom transition from the tile's frame; Backspace, Esc or ⌘↑ goes up with the reverse transition; the arrow keys and h/j/k/l move the selection between tiles; Space stages (FR-KB-5); right-click opens Add to Queue, Reveal in Finder, Quick Look, Show Recommendation (when matched) and Get Info (opens the inspector).
- **FR-EXP-9** Files are not drilled into. Double-clicking a file reveals it in Finder.

#### 13.4.6 Recommended

- **FR-REC-1** A full-height page with the two v1 sections ("Safe to remove", "Review first"), the same rows, blocked states and sorting. The list scrolls inside the page and fades at its bottom edge; no row is ever clipped mid-height.
- **FR-REC-2** Selecting a row (not its checkbox) fills the inspector with the v1 drawer content (section 7.5).
- **FR-REC-3** Copy script leaves the footer. Export Cleanup Script… lives in the File menu and the row context menu, and saves a `.sh` file through the save panel; Copy to Clipboard stays as a context menu item.
- **FR-REC-4** The page footer shows the queue summary (`52.5 GB staged · 5 items`) and Select safe.
- **FR-REC-5** Rows show badges from the item: "Active work" (section 13.4.11) and, with Pro, "Reproducible via Cargo.lock" (section 13.10.1). j/k and the arrows move between rows, Space stages, Return opens the inspector.

#### 13.4.7 Inspector

- **FR-INS-1** A trailing panel, 300 pt wide, toggled from the toolbar and ⌥⌘I. It shows whatever is selected: a tile, a recommendation row, or the "smaller items" list.
- **FR-INS-2** For a folder: name, full path, size, file count, category and its description, the rule description and "what happens if removed" when matched, the five largest subfolders, and Reveal in Finder.
- **FR-INS-3** For a recommendation: the v1 drawer content, including per-item checkboxes and blocked reasons.

#### 13.4.8 Cleanup

- Behavior is unchanged from section 7.6, including the confirm button that Enter never triggers.
- The confirm, progress and results views become one sheet that slides down from the toolbar over a dimmed window, and slides back up when dismissed.
- The results view ends with "Updating results…" while the follow-up background scan runs (FR-BG-6).
- When free space grew by less than was deleted and local snapshots exist, the results view says "18.2 GB is still held by 3 local Time Machine snapshots" and links to the snapshot inspector (section 13.10.5). This explanation is free.
- The sheet's confirm view lists the queue (FR-Q-3) and offers Copy as Shell Script next to the confirm button.

#### 13.4.9 Keyboard and command palette

- **FR-KB-1** ⌘K opens the command palette from any page (not during onboarding or while a sheet is open). It is a search field over a list: ↑/↓ or ⌃N/⌃P move, Return runs, Esc closes. Each command shows its shortcut when it has one.
- **FR-KB-2** One fuzzy list over: pages; folders in the current snapshot by name and path ("Go to ~/Projects/atlas"); recommendation rows ("Show Xcode DerivedData"); and app commands (Rescan, Select Safe Items, Export Cleanup Script…, Show Inspector, Highlight Cleanable). Rules match on their label, ID and `keywords` (a new rule field, added to RULES.md section 3 in Phase 14), so "dormant rust targets" finds the Rust `target` rule.
- **FR-KB-3** Typing `>` limits the list to commands. `> filter` takes one or more terms: `size>1GB`, `size<100MB`, `tier:safe`, `tier:review`, `category:<id>`, `idle>30d`. An active filter shows as a removable token in the toolbar. Explore dims tiles that don't match (like Highlight cleanable) and Recommended hides rows that don't match.
- **FR-KB-4** Free: everything in FR-KB-1 to FR-KB-3, plus staging or unstaging one item. Pro (with the "Pro" tag in the list): batch staging by query, such as `> stage dormant rust targets` (all ready items of a rule), `> stage safe size>1GB` or `> stage category:ai_models`; saved filters; and "Run sweep rule now" (section 13.10.7). Batch commands only stage (S-11). Cleaning still goes through the sheet.
- **FR-KB-5** Explore keymap, active while the treemap has focus and no text field is focused:

  | Key | Action |
  |-----|--------|
  | h/←, j/↓, k/↑, l/→ | Move the selection to the nearest tile in that direction (by tile centre) |
  | Space | Stage or unstage the selected tile (FR-Q-2) |
  | Return | Open the folder (zoom); on a file, Reveal in Finder |
  | Backspace, Esc, ⌘↑ | Up one level (Esc first closes an open menu or popover) |
  | ⌘Y | Quick Look the selected item (Finder's other Quick Look key; Space is taken by staging) |
  | / | Command palette limited to folders under the current one |

  Tab still moves focus between the sidebar, toolbar, content and inspector. Recommended uses j/k and the arrows for rows, with the same Space and Return.
- **FR-KB-6** Help › Keyboard Shortcuts lists every shortcut.

#### 13.4.10 Staging queue

- **FR-Q-1** The selection becomes the staging queue: one list of everything staged from Explore, Recommended and the palette. It is free. S-09 still holds: the queue starts empty on every launch and is never saved to disk.
- **FR-Q-2** Items are staged with a row checkbox, Space on a tile or row, Add to Queue in the context menu, or a palette command. Only rule-matched items can be staged (S-01). Space on a folder that contains recommended items stages its ready items; on a tile with nothing cleanable, the toolbar status says "Nothing here can be cleaned".
- **FR-Q-3** The toolbar queue button shows the count and total (`5 · 52.5 GB`) and opens the queue in the inspector: items grouped by rule with size, method and a remove button, then Clear Queue, Copy as Shell Script, Export Cleanup Script… and Clean up.
- **FR-Q-4** Copy as Shell Script builds the plan and copies the `export_script` output. The script runs the same methods the app would (section 7.7): `rm -rf --` for deletes, a Finder delete for Trash items, and the tool's own command for command rules. It is not rewritten to `rm -rf` everywhere, so the script and the app stay equivalent (G4).
- **FR-Q-5** The queue carries over a snapshot swap by `(rule_id, path)`, like the selection (FR-BG-4).

#### 13.4.11 Active-work guard and project artifacts

Free, because it is a safety check.

- **FR-GIT-1** A new guard, `git_active` (added to RULES.md section 4.2 in Phase 14), on project artifact rules. For the item's repository (below the scan root) it detects uncommitted or untracked changes, and commits that are on no remote branch (a branch ahead of its upstream, or a branch with no upstream whose commits are on no remote). It runs in process with libgit2 (`git2` crate), with no `PATH` lookup and no subprocess.
- **FR-GIT-2** `git_active` belongs to a new guard class, *demoting*. An item that would be safe-tier moves to review, and each affected item gets the amber badge "Active work" with the reason ("3 uncommitted files", "2 unpushed commits on feat/search"). An old last-modified time does not override it. The item stays cleanable after review.
- **FR-GIT-3** A check that cannot complete counts as active work.
- **FR-GIT-4** New project artifact rules for idle projects, each with its own project markers and idle evidence: Rust `target` (`Cargo.toml`, `Cargo.lock`), Python `.venv` and `venv` (`pyproject.toml`, `requirements.txt`, `uv.lock`), and `dist` (`package.json`, review tier). The existing `node-modules-idle` and `next-turbo-build` rules get `git_active` too. Each rule gets `keywords` for the palette (for example "dormant", "stale", "idle").

### 13.5 Visual specification (v2)

**Window**
- Default 1280×820, minimum 960×640. Transparent titlebar with the traffic lights inset into the sidebar.
- macOS: `WindowBackgroundAppearance::Blurred`, with the sidebar showing the blur through a translucent fill and the content area on an opaque fill. GPUI has no per-element backdrop blur, so cards are semi-transparent fills over the window, never nested glass on glass.
- Later platforms: Windows uses `MicaBackdrop`; elsewhere the opaque fallback.

**Typography**
- The system font (`.SystemUIFont`, SF Pro on macOS) replaces Geist. Numbers use tabular figures (`FontFeatures` `tnum`) instead of a monospaced font.
- Paths use the system monospaced font. Geist Mono stays only as a fallback if GPUI can't resolve it (checked in Phase 8).
- Scale (pt): large number 28 semibold, page title 20 semibold, headline 13 semibold, body 13 regular, secondary 11 regular. Tracking follows the system tables; no custom letter spacing.

**Shape and spacing**
- Radius: widgets 14, tiles 10, controls and rows 8. Spacing on a 4 pt grid: 16 between widgets and around the content, 12 inside widgets, 4 between tiles.
- No 1 px card borders; surfaces separate by fill. Hairlines only for list separators and the sidebar edge.

**Color**
- Accent: system blue (`#007AFF` light, `#0A84FF` dark). One accent-filled control per view, the Clean up button, which uses the success green when enabled.
- Status: safe uses system green (`#34C759` / `#30D158`), review uses system orange (`#FF9500` / `#FF9F0A`). Text on tinted fills uses a darker shade of the same hue and must pass WCAG AA (fixes the open Phase 5 contrast finding).
- Categories keep their six IDs in `rules.toml`; their `fill`, `border`, `fill_dark` and `border_dark` values are replaced with a muted palette tuned for large tiles. Tiles use the fill only; the border colors become the sidebar dots and the capacity bar segments.
- Light and dark appearance follow the system, as in v1.

**Texture (semantic treemap)**
- A tile's surface tells what kind of space it is, so reclaimable "ghost space" stands out before any label is read:

  | Kind | When | Surface |
  |------|------|---------|
  | Disposable | The tile is a ready safe-tier item (caches, build output) | Category fill at reduced opacity with 45° stripes in the category border color |
  | Restorable | The tile is a review-tier item | Category fill at reduced opacity, no stripes |
  | Kept | Anything else that is not system-managed (user data, projects) | Solid category fill |
  | System-managed | FR-EXP-6 | Neutral fill with a lock glyph |
  | Smaller items | FR-EXP-2 | Neutral fill, dashed outline |

- Only the item itself is textured. A folder that only contains cleanable items stays solid and shows its cleanable line (FR-EXP-5), so a texture always means "this whole tile".
- Stripes use GPUI's `pattern_slash` (as the v1 hatch does) at no more than 20% opacity. Label text passes WCAG AA against the darkest stripe pixel in both themes, checked by a test. GPUI has no custom shader API, and none is needed.
- With Reduce Transparency the translucent fills become solid lighter fills. Stripes stay, because they carry the meaning.
- Highlight cleanable (FR-EXP-7) dims Kept and System-managed tiles. A one-line texture key sits under the treemap.

**Motion**
- Springs with damping 1.0 and response 0.35 s for navigation (drill-down zoom, inspector, sheet), 0.25 s for small state changes. No overshoot: nothing in DiskLens is thrown.
- Enter and exit use the same path: the zoom grows from the tile and shrinks back into it; the sheet comes down from the toolbar and goes back up.
- Reduce Motion (read from `NSWorkspace.accessibilityDisplayShouldReduceMotion`) replaces movement with a 150 ms cross-fade.

**Icons**
- Line icons drawn as SVG in `assets/icons/ui/`, in the style of SF Symbols at regular weight. SF Symbols themselves are not bundled (their license limits use outside Apple's own frameworks).

### 13.6 Free and Pro

Rule: the free tier never stops someone from understanding their disk or cleaning what DiskLens found. Every v1 feature stays free. Pro adds new capabilities.

| Free | Pro |
|------|-----|
| Scan of the home folder, background rescans | Lockfile rehydration tags and orphaned global-store packages (13.10.1) |
| Overview, Explore, inspector, semantic textures | AI models hub (13.10.2) |
| Every recommendation, both tiers, with cleanup | Inside containers and VM disk images (13.10.3) |
| Developer providers (Docker, simulators, Ollama, git worktrees) | Extra Apple, Android and creative scratch scanners (13.10.4) |
| Active-work git guard and the new project artifact rules (13.4.11) | APFS snapshot inspector with thin and delete (13.10.5) |
| Command palette: navigation, filters, single-item staging (13.4.9) | Palette batch commands and saved filters (FR-KB-4) |
| Keyboard navigation (FR-KB-5) | Menu bar agent: free space, low-disk and write-burst alerts (13.10.6) |
| Staging queue, Copy as Shell Script, script export and audit log | Auto-sweep rules that stage and notify (13.10.7) |
| "Held by local snapshots" explanation | Scans of other folders, volumes and external drives |
| Light and dark appearance, updates | History: what grew since the last scan, a snapshot comparison per folder |
| | Large and old files finder |
| | Duplicate finder |
| | User-editable local rules file |
| | Clone-aware sizes (`ATTR_CMNEXT_PRIVATESIZE`) |

**Gate rules**
- A Pro feature shows its entry point with a small "Pro" tag. Using it opens one sheet that explains the feature and offers purchase or license entry. No countdowns, nags at launch, or blurred results.
- Safety never depends on the license. The free and Pro builds are one binary. Guards, revalidation and protections run the same with and without a license.
- Pro scanners add recommendations only while Pro is active. Without a license, their rules are skipped by the matcher (`pro = true`, a new rule field added in Phase 15), so a free user never sees a result they can't act on. One quiet "More with Pro" entry at the end of Recommended says what the Pro scanners look for, without sizes.
- A rule that is free in v1 stays free when a Pro scanner extends the same data (Ollama models, Docker, unavailable simulators, the pnpm store).
- Licensing is checked offline: a license is a signed token verified against a public key in the app, the same approach as the update signature. No account is required.

**Distribution:** direct download, as in v1 (the App Store sandbox blocks scanning). The payment provider and the price are open decisions (section 13.8).

### 13.7 Platform direction

GPUI stays the UI stack because Windows and Linux builds are planned after v2. To keep that door open:
- Platform APIs stay behind `platform/` traits (as `System`, `Runner`, `Trash` do now).
- Chrome differences live in one UI module: traffic-light inset, ⌘ or Ctrl in shortcuts, the menu bar or an in-window menu, the window material.
- Text and examples avoid macOS-only wording where a neutral word works.
- macOS-only Pro features (APFS snapshots, the menu bar agent's AppKit status item, Quick Look) sit behind `platform/` and are hidden where the platform lacks them.

### 13.8 Open decisions

| Decision | Options |
|----------|---------|
| Pro pricing | One-time purchase with paid major upgrades, or a low yearly price |
| Payment and license provider | Paddle, Lemon Squeezy, or another merchant of record |
| Trial | Pro features fully usable for 14 days, or no trial |
| Which Pro features ship with v2.0 | Recommended first set: snapshot inspector, containers and VMs, AI models hub, lockfile tags and orphaned stores, palette batch commands. They extend the main window and need no background process. Then the menu bar agent and auto-sweep (auto-sweep depends on the agent), then other volumes, history and the rest in point releases |
| Privileged snapshot deletion mechanism | A one-shot `osascript` `do shell script … with administrator privileges` (works with ad-hoc signing, but goes through a shell), or an `SMAppService` privileged helper (needs Developer ID and user approval in System Settings). Decided by the Phase 17 spike |
| Developer ID | Still needed from v1 (PLAN.md Phase 6); required before selling |

### 13.9 Release criteria for v2

- All v1 release criteria still hold, including signing and notarization.
- Goals V2-G1 to V2-G4 and V2-G6 are checked and recorded in `docs/QA.md`.
- If the menu bar agent ships, its resident memory stays under 15 MB over a 24 hour run.
- The app matches `docs/mockup-v2.png` with mock data, judged side by side, in light and dark.
- No clipped or overlapping text in the treemap at the minimum window size, checked with the mock and with a real home scan.
- The free tier works with no license present; a test proves that every cleanup path ignores the license state.

### 13.10 Pro features

Each Pro feature has its own subsection here, as PLAN.md requires before work starts. Paths marked "verify" are checked against the installed tool in that feature's phase before a rule is written.

#### 13.10.1 Lockfile tags and orphaned global stores

- **FR-LOCK-1** Project artifact items (`node_modules`, `.next`, `.turbo`, `target`, `dist`, `.venv`) show "Reproducible via <lockfile> (4.2 GB)" when a lockfile sits next to the project manifest: `bun.lock`, `bun.lockb`, `pnpm-lock.yaml`, `package-lock.json`, `yarn.lock`, `Cargo.lock`, `uv.lock`, `poetry.lock`, `Pipfile.lock`. Without one, the item shows "No lockfile: a reinstall may pick newer versions". The rule's plain `regenerates` text stays free.
- **FR-STORE-1** Global stores are checked for packages no local project uses, each with that store's own model:
  - **pnpm store** (`~/Library/pnpm/store`, `~/.pnpm-store`): pnpm hard-links store files into `node_modules`, so a file with a link count of 1 is used by no project. The scanner already tracks link counts, so the orphaned size is measured exactly instead of estimated. Cleanup stays `pnpm store prune` (owning tool, RULES.md principle 3).
  - **Bun cache** (`~/.bun/install/cache`): on macOS Bun installs by clonefile, so link counts say nothing. A cached `name@version` is orphaned when no `bun.lock` under the scan root references it. A project with only the binary `bun.lockb` counts every package as used (conservative). The orphaned folders are deleted.
  - **Cargo** (`~/.cargo/registry/cache`, `~/.cargo/registry/src`): crates referenced by no `Cargo.lock` under the scan root. Their `.crate` files and source folders are deleted; Cargo downloads them again when needed.
  - **Homebrew** (`~/Library/Caches/Homebrew`): Homebrew downloads are not linked to repositories, so "orphaned" means old versions and downloads that `brew cleanup --dry-run` lists. Cleanup is `brew cleanup`.
- **FR-STORE-2** Orphans are review tier ("Not used by any project on this Mac"), because projects outside the scan root or on other volumes may still use them.

#### 13.10.2 AI models hub

- **FR-AI-1** A Models page lists local weights from Ollama (`~/.ollama/models`), the Hugging Face hub cache (`~/.cache/huggingface/hub`) and LM Studio (`~/.lmstudio/models`, `~/.cache/lm-studio/models`).
- **FR-AI-2** Models are grouped by family (Llama, Qwen, Mistral, Gemma and so on), then by variant with parameter count and quantization (`Q4_K_M`, `Q8_0`, `F16`, MLX 4-bit). These come from the Ollama manifest and config blob, the Hugging Face `config.json`, and file names. Weight files themselves are never opened, in line with the "contents are never read" rule (section 11).
- **FR-AI-3** Last used: for Ollama, the newest load in `~/.ollama/logs/server*.log`; otherwise the newest access time under the model. APFS updates access times lazily, so atime-based values read "Last used about 3 weeks ago", and "Unknown" when there is no signal.
- **FR-AI-4** Removal uses the owning tool where it exists: `ollama rm`; the `hf` CLI's cache delete when installed, otherwise the whole `models--<org>--<name>` folder (never a single snapshot folder, because snapshots share blobs through symlinks). Review tier, with the existing `process_not_running` and `requires_running` guards.

#### 13.10.3 Containers and virtual machines

- **FR-VM-1** Disk images are found and sized by allocated size (they are sparse): Docker Desktop `Docker.raw` (`~/Library/Containers/com.docker.docker/Data/vms/0/data/`), OrbStack's data image (verify, under `~/Library/Group Containers/*.dev.orbstack/`), Colima (`~/.colima/_lima/<profile>/diffdisk`, verify) and Lima (`~/.lima/<vm>/diffdisk`).
- **FR-VM-2** For engines with a Docker API, DiskLens calls `GET /system/df` on the engine's unix socket (`~/.docker/run/docker.sock`, `~/.orbstack/run/docker.sock`, `~/.colima/<profile>/docker.sock`). The request is a plain HTTP GET over a `UnixStream`: no shell, and no `docker` binary needed.
- **FR-VM-3** In Explore the image tile gets virtual children: Images (unused, dangling), Containers (stopped), Volumes (dangling), Build cache, and "Free inside the image" (allocated on disk but unused inside the VM). They drill down like folders and are marked as virtual in the inspector.
- **FR-VM-4** Actions go through the same API: remove dangling and unused images, prune build cache, remove stopped containers. Dangling volumes are review tier, one item per volume, never part of Select safe ("A volume may hold the only copy of a database").
- **FR-VM-5** Honesty about space: freeing space inside a VM doesn't shrink its disk image right away. Docker Desktop and OrbStack give the space back on their own over time; Colima and Lima need `fstrim` inside the VM. The results say "Freed inside OrbStack. The disk image shrinks when OrbStack reclaims it."
- **FR-VM-6** When the engine is not running, the image is one tile with "Start OrbStack to see inside". v1's `docker-unused` rule stays free and unchanged.

#### 13.10.4 Apple, mobile and creative scratch data

- **FR-DEV-1** Beyond the v1 rules (which stay free):
  - simulator runtimes no installed Xcode uses (`xcrun simctl runtime list -j`, removed with `xcrun simctl runtime delete <id>`)
  - Xcode `DerivedData` per project, with the last build date. A folder whose workspace (`WorkspacePath` in its `info.plist`) no longer exists is safe tier.
  - old Gradle wrapper distributions (`~/.gradle/wrapper/dists`) that no project's `gradle-wrapper.properties` under the scan root uses
- **FR-CRE-1** Creative caches, each guarded by its app not running:
  - Final Cut Pro, inside `*.fcpbundle` libraries: `Render Files` (safe) and `Transcoded Media` (optimized `High Quality Media` and `Proxy Media`, review: "Rebuilt from the original media. If the originals are missing or on a drive that isn't connected, this may be the only copy").
  - DaVinci Resolve `CacheClip` folders (default `~/Movies/CacheClip`, verify), safe.
  - Adobe `Media Cache Files` and `Media Cache` (`~/Library/Application Support/Adobe/Common/`), cleaned together, safe.
  - Figma desktop caches (verify the path), safe.
- **FR-CRE-2** Libraries and caches on external scratch drives need the Pro "other volumes" scan. On the boot volume they work with the home scan.
- **FR-CRE-3** Idle evidence for creative projects is the library or project file's modification time, which auto-sweep uses (13.10.7).

#### 13.10.5 APFS snapshot inspector

- **FR-SNAP-1** Lists local Time Machine snapshots of the boot volume (`tmutil listlocalsnapshots /`, names `com.apple.TimeMachine.<date>.local`) with date and age. Other APFS snapshots (`diskutil apfs listSnapshots`) are listed read-only with "Managed by macOS" (S-12).
- **FR-SNAP-2** macOS does not report a size per snapshot. The inspector shows the volume's purgeable total (important-usage available capacity minus `statfs` free; FR-SCAN-7 already reads both) as "Up to 38 GB is purgeable, mostly held by local snapshots", and per snapshot only its date.
- **FR-SNAP-3** Free: the explanation in the cleanup results (section 13.4.8) and a "Held by snapshots" line in the Overview Disk widget.
- **FR-SNAP-4** Pro actions:
  - **Thin:** `tmutil thinlocalsnapshots / <bytes> <urgency>` asks macOS to free that much purgeable space. If the Phase 17 spike shows it needs no admin rights, it runs without a prompt.
  - **Delete one snapshot:** `tmutil deletelocalsnapshots <date>`, which needs admin rights on current macOS.
- **FR-SNAP-5** Privileged actions (the S-06 exception):
  - Opt-in. The option stays off until the user turns on "Allow actions that need administrator rights" in the sheet that explains what will run and why (also in Settings).
  - Each action shows the exact command, then the system administrator prompt. No authorization is kept, so the next action asks again.
  - The command is fixed argv for `/usr/bin/tmutil`. Its only variable is a snapshot date that must match `^\d{4}-\d{2}-\d{2}-\d{6}$` and be in the `listlocalsnapshots` output read right before. If the chosen mechanism (section 13.8) uses a shell, the command string is built only from constants and that validated date. That is the single documented exception to S-05.
  - Dry run applies. Every action is audit logged with method `privileged_command`.

#### 13.10.6 Menu bar agent and write bursts

- **FR-AGT-1** A separate small binary, `disklens-agent`, inside the app bundle, registered as a login item through `SMAppService` (macOS 13 or later). It is on only when the user turns it on. It uses an AppKit status item through `objc2`, with no GPUI. Target: under 15 MB resident memory and near-zero CPU at idle.
- **FR-AGT-2** The menu bar shows free space. Its menu shows free space, last scan age, the total waiting in sweep results, Open DiskLens and Rescan.
- **FR-AGT-3** Low-disk alert below a threshold (setting, default 10 GB free).
- **FR-AGT-4** Write bursts: the agent samples free space every 10 s and per-process bytes written (`proc_pid_rusage`, `ri_diskio_byteswritten`, available for the user's own processes without admin) every 30 s. When the volume loses more than 10 GB in 5 minutes (setting), it alerts with the top writer ("Xcode wrote 9.2 GB in the last 5 minutes") and, when an FSEvents stream on the scan root points to one, the folder growing fastest. Alert actions: Show in DiskLens (opens Explore at that folder) and Ignore This App.
- **FR-AGT-5** The agent never removes anything (S-11). Notification permission is asked when the user first turns on an alert (FR-ONB-6).

#### 13.10.7 Auto-sweep

- **FR-SWP-1** Sweep rules are deterministic and built from existing recommendation rules. They never add new targets. A sweep rule names a rule or rule group, a scope glob under the scan root, a condition, and a schedule:
  - "Clear `node_modules` and `.next` in `~/Projects/*` when the project has had no commits for 45 days."
  - "Clear Final Cut Pro render files for libraries not opened in 30 days."
  - Schedule: daily or weekly at a time, or when free space falls below a threshold.
- **FR-SWP-2** A sweep's idle threshold is at least the rule's own. Every guard applies, and items demoted by `git_active` are never swept.
- **FR-SWP-3** A sweep run rescans the scope, matches, and then stages and notifies: "Sweep ready: 12.4 GB in 8 projects". Clicking the notification opens DiskLens with exactly those items in the queue and the cleanup sheet open. Confirmation is as usual (S-11). Because the user opens it explicitly, this does not break S-09's empty default. An unconfirmed sweep result is replaced at the next run.
- **FR-SWP-4** Sweeps run in the agent (13.10.6), so they work while the main window is closed. Each sweep run is written to the audit log.
