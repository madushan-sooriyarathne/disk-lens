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
- The app never asks for an administrator password.

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
- **S-06** No elevated privileges are requested or used.
- **S-07** Every action is logged with timestamp, rule, path, method, bytes and result.
- **S-08** Dry-run mode (`DISKLENS_DRY_RUN=1`, default on in debug builds) logs actions without performing them.
- **S-09** The default selection is empty.
- **S-10** A directory containing a `.git` directory (a repository root) is never deleted by path.

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

## 12. Release criteria for v1

- All safety tests pass, including the adversarial fixture suite.
- Scanner accuracy is within 1% of `du -sk` on three real folders.
- Every rule in RULES.md has a match test and a non-match test.
- The UI matches the mockup with mock data, judged side by side.
- The app is signed, notarized, and launches on a clean macOS 13 machine.

## 13. Version 2

v2 keeps the v1 engine (scanner, rules, safety, cleanup) and rebuilds the experience around it. Four themes:

1. A proper onboarding flow.
2. Rescans in the background, over the previous results, instead of a blocking scanning screen.
3. A complete UI overhaul that follows Apple's Human Interface Guidelines for macOS, so the app feels as if Apple built it.
4. A simpler treemap that an average user can read.

v2 also splits the product into a free tier and a Pro tier (section 13.6).

**Design references:** the HIG pages for [design principles](https://developer.apple.com/design/human-interface-guidelines/design-principles), [designing for macOS](https://developer.apple.com/design/human-interface-guidelines/designing-for-macos), [foundations](https://developer.apple.com/design/human-interface-guidelines/foundations), [patterns](https://developer.apple.com/design/human-interface-guidelines/patterns), [components](https://developer.apple.com/design/human-interface-guidelines/components), [inputs](https://developer.apple.com/design/human-interface-guidelines/inputs) and [technologies](https://developer.apple.com/design/human-interface-guidelines/technologies). Every design task uses the `apple-design` skill. `docs/mockup-v2.png` (produced in Phase 8) becomes the visual target and replaces `docs/mockup.jpg` for v2 work.

### 13.1 Goals

| ID | Goal | Measure |
|----|------|---------|
| V2-G1 | Results are always on screen | After the first scan, no screen ever replaces the results while a scan runs |
| V2-G2 | Readable at a glance | Every visible treemap tile shows its full name or no text at all; no clipped or overlapping text at any window size down to the minimum |
| V2-G3 | Feels native | Side-by-side review against Finder and System Settings › Storage finds no off-platform spacing, type or control |
| V2-G4 | Understandable by a non-developer | In a hallway test, 4 of 5 people can say how much they can safely free within 10 seconds of the Overview appearing |
| V2-G5 | Fair free tier | Everything v1 does stays free; Pro only adds new capabilities |

### 13.2 Non-goals for v2

- Windows and Linux builds. GPUI stays the UI stack so they remain possible later, so v2 keeps platform-specific code behind `platform/` and the few UI chrome differences (section 13.5) in one place.
- Mac App Store distribution (unchanged reason: the sandbox blocks scanning).
- A native AppKit or SwiftUI rewrite.
- Changing safety rules S-01 to S-10. v2 adds rules; it never relaxes one.

### 13.3 User stories

| ID | Story | Acceptance |
|----|-------|------------|
| US-20 | On first launch a short welcome tells me what DiskLens does and that nothing is deleted without my confirmation | Three short points, one Continue button, under 10 seconds to read |
| US-21 | Onboarding helps me grant Full Disk Access, and moves on by itself when I do | Granting access in System Settings advances the step when the window regains focus; "Continue without" is always available |
| US-22 | My first scan shows categories filling in as they are found | The first scan is the only one that uses a full-window progress view |
| US-23 | When I rescan, I keep working with the previous results | Toolbar shows progress and Cancel; results swap in place when the scan finishes |
| US-24 | When I relaunch, I see my last results at once and they refresh on their own when stale | Snapshots older than 24 hours start a background rescan on launch |
| US-25 | The Overview tells me how much I can free safely, in one number | "Safe to clean" widget with total, item count and Select safe |
| US-26 | The treemap shows one level at a time and every label is readable | Rules in section 13.4.5 |
| US-27 | I can tell which folders I can't do anything about | System-managed folders are gray with a lock |
| US-28 | Selecting a tile or a recommendation shows details beside it | Inspector on the trailing side, toggled from the toolbar |
| US-29 | After a cleanup, cleaned items leave the screen right away | Removed before the follow-up background rescan finishes |
| US-30 | Pro features are visible but never stand between me and cleaning what was found | Section 13.6 gate rules |

### 13.4 Functional requirements

#### 13.4.1 Onboarding

- **FR-ONB-1** Shown when `onboarding_done` is false. Three steps in one window-sized card with a page indicator: Welcome, Full Disk Access, First scan.
- **FR-ONB-2** Welcome: app icon, title, three short points (see what fills your disk; find space that is safe to free; nothing is removed without your confirmation, and most items go to the Trash). One Continue button.
- **FR-ONB-3** Full Disk Access: why it is needed in one sentence, an Open System Settings button (deep link from section 7.8), a small illustration of the toggle, and "Continue without". Access is rechecked on every window activation; once granted the step shows a check mark and advances after a short pause.
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
- **FR-WIN-2** Toolbar, left to right: sidebar toggle; back and forward (Explore only); the page title, or the breadcrumb on Explore; flexible space; scan status (FR-BG-2); inspector toggle; the primary button `Clean up <selected total>`.
- **FR-WIN-3** The primary button stays in place. With nothing selected it reads "Clean up" and is disabled; the default selection stays empty (S-09).
- **FR-WIN-4** The app has a full menu bar: File (Rescan ⌘R, Export Cleanup Script… ⌘E, Reveal in Finder ⌘⇧R), Edit (Select Safe Items, Deselect All), View (Overview ⌘1, Explore ⌘2, Recommended ⌘3, Show Inspector ⌥⌘I, Highlight Cleanable ⌘⇧H, Show Sidebar ⌃⌘S), Window, Help.
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
- **FR-EXP-4** Tiles have 4 pt gaps, a 10 pt corner radius, no borders, and a fill from the category color (section 13.5).
- **FR-EXP-5** A tile that contains recommended items shows a second line "9.1 GB cleanable" in place of the plain size line when it fits. The diagonal hatch is removed.
- **FR-EXP-6** System-managed folders (protected paths from RULES.md section 5, and folders under `~/Library` with no recommendation anywhere inside) use the neutral fill with a lock glyph and the tooltip "Managed by macOS or an app".
- **FR-EXP-7** "Highlight cleanable" (toolbar menu and ⌘⇧H) dims every tile without cleanable content. It replaces the v1 Category/Cleanup mode toggle.
- **FR-EXP-8** Interaction: click selects a tile and fills the inspector; double-click or Return opens a folder with a zoom transition from the tile's frame; Esc or ⌘↑ goes up with the reverse transition; the arrow keys move the selection between tiles; right-click opens Reveal in Finder, Show Recommendation (when matched) and Get Info (opens the inspector).
- **FR-EXP-9** Files are not drilled into. Double-clicking a file reveals it in Finder.

#### 13.4.6 Recommended

- **FR-REC-1** A full-height page with the two v1 sections ("Safe to remove", "Review first"), the same rows, blocked states and sorting. The list scrolls inside the page and fades at its bottom edge; no row is ever clipped mid-height.
- **FR-REC-2** Selecting a row (not its checkbox) fills the inspector with the v1 drawer content (section 7.5).
- **FR-REC-3** Copy script leaves the footer. Export Cleanup Script… lives in the File menu and the row context menu, and saves a `.sh` file through the save panel; Copy to Clipboard stays as a context menu item.
- **FR-REC-4** The page footer shows the selection summary (`52.5 GB selected · 5 items`) and Select safe.

#### 13.4.7 Inspector

- **FR-INS-1** A trailing panel, 300 pt wide, toggled from the toolbar and ⌥⌘I. It shows whatever is selected: a tile, a recommendation row, or the "smaller items" list.
- **FR-INS-2** For a folder: name, full path, size, file count, category and its description, the rule description and "what happens if removed" when matched, the five largest subfolders, and Reveal in Finder.
- **FR-INS-3** For a recommendation: the v1 drawer content, including per-item checkboxes and blocked reasons.

#### 13.4.8 Cleanup

- Behavior is unchanged from section 7.6, including the confirm button that Enter never triggers.
- The confirm, progress and results views become one sheet that slides down from the toolbar over a dimmed window, and slides back up when dismissed.
- The results view ends with "Updating results…" while the follow-up background scan runs (FR-BG-6).

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
| Scan of the home folder, background rescans | Scans of other folders, volumes and external drives |
| Overview, Explore, inspector | History: what grew since the last scan, a snapshot comparison per folder |
| Every recommendation, both tiers, with cleanup | Scheduled safe-tier cleanup |
| Developer providers (Docker, simulators, Ollama, git worktrees) | Menu bar free-space indicator and low-disk alerts |
| Script export and audit log | Large and old files finder |
| Light and dark appearance, updates | Duplicate finder |
| | User-editable local rules file |
| | Clone-aware sizes (`ATTR_CMNEXT_PRIVATESIZE`) |

**Gate rules**
- A Pro feature shows its entry point with a small "Pro" tag. Using it opens one sheet that explains the feature and offers purchase or license entry. No countdowns, nags at launch, or blurred results.
- Safety never depends on the license. The free and Pro builds are one binary.
- Licensing is checked offline: a license is a signed token verified against a public key in the app, the same approach as the update signature. No account is required.

**Distribution:** direct download, as in v1 (the App Store sandbox blocks scanning). The payment provider and the price are open decisions (section 13.8).

### 13.7 Platform direction

GPUI stays the UI stack because Windows and Linux builds are planned after v2. To keep that door open:
- Platform APIs stay behind `platform/` traits (as `System`, `Runner`, `Trash` do now).
- Chrome differences live in one UI module: traffic-light inset, ⌘ or Ctrl in shortcuts, the menu bar or an in-window menu, the window material.
- Text and examples avoid macOS-only wording where a neutral word works.

### 13.8 Open decisions

| Decision | Options |
|----------|---------|
| Pro pricing | One-time purchase with paid major upgrades, or a low yearly price |
| Payment and license provider | Paddle, Lemon Squeezy, or another merchant of record |
| Trial | Pro features fully usable for 14 days, or no trial |
| Which Pro features ship with v2.0 | Recommended first set: history, menu bar indicator, other volumes. The rest follow in point releases |
| Developer ID | Still needed from v1 (PLAN.md Phase 6); required before selling |

### 13.9 Release criteria for v2

- All v1 release criteria still hold, including signing and notarization.
- Goals V2-G1 to V2-G4 are checked and recorded in `docs/QA.md`.
- The app matches `docs/mockup-v2.png` with mock data, judged side by side, in light and dark.
- No clipped or overlapping text in the treemap at the minimum window size, checked with the mock and with a real home scan.
- The free tier works with no license present; a test proves that every cleanup path ignores the license state.
