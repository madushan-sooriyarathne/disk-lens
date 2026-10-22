# DiskLens: Product Requirements

Version 1.0 · Owner: Madushan · Status: Ready for implementation

Related documents: `docs/RULES.md` (classification and cleanup rules), `docs/PLAN.md` (implementation plan).

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

| Card | Value | Secondary |
|------|-------|-----------|
| Free after cleanup | available space + selected total | green delta `+52.5 GB` equal to the selected total; hidden when 0 |
| Safe to remove | sum of ready safe-tier items | item count |
| Needs review | sum of ready review-tier items | item count |
| Not scanned | volume total − available − scanned home size, clamped at 0 | label "system & apps" |

Sizes marked as estimates (see RULES.md) show as `up to 4.1 GB`. Totals never double count: each path belongs to at most one recommendation.

### 7.3 Explorer (treemap)

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

- Title "Recommendations" with subtitle "Select items to build a cleanup script".
- Two sections, each with a tinted pill ("Safe to remove" green, "Review first" amber) and its tier total on the right.
- Each row shows a checkbox, label, a mono subtitle of paths (truncated with ellipsis, full list in a tooltip), size, and a chevron.
- Blocked rows show a disabled checkbox and a short reason such as "Quit Xcode to clean".
- The footer shows `52.5 GB selected · 5 items` with three buttons: "Select safe", "Copy script" (secondary) and "Clean selected" (primary, disabled when nothing is selected).

**Change from the mockup:** "Clean selected" is added as the primary action, and "Copy script" becomes secondary.

### 7.5 Detail drawer

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

- **S-01** Only items produced by the rules engine for the current snapshot can be cleaned. The frontend sends item IDs, never paths.
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

## 13. Future (v2 candidates)

- Large and old files finder for Downloads
- Duplicate finder
- Clone-aware sizes
- Menu bar free-space indicator
- Scheduled safe-tier cleanup
- User-editable local rules file
