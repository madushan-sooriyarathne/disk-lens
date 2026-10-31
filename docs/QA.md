# DiskLens: Manual QA checklist

Run on macOS against the fixture sandbox unless a step says otherwise. Rebuild it first with `scripts/make-fixture.sh`. Debug builds are dry runs by default (`DISKLENS_DRY_RUN=1`); steps marked **real** set `DISKLENS_DRY_RUN=0`.

```bash
DISKLENS_SCAN_ROOT=$PWD/.sandbox/home cargo run                      # dry run
DISKLENS_SCAN_ROOT=$PWD/.sandbox/home DISKLENS_DRY_RUN=0 cargo run   # real, sandbox only
DISKLENS_MOCK=preselect cargo run                                    # mock data
```

The audit log of sandboxed runs is `.sandbox/logs/cleanup.log`; of real-home runs `~/Library/Logs/DiskLens/cleanup.log`.

## Scan and explorer

- [x] First launch with no snapshot shows the idle screen; Scan starts the scan and the progress screen animates.
- [x] Cancel during a scan returns to the previous state with a notice.
- [x] The treemap shows the sandbox; click drills down, the breadcrumb goes back, Esc goes up one folder.
- [x] Category / Cleanup toggle recolors tiles; matched tiles are hatched.
- [x] Hover shows the tooltip; right-click opens the context menu; Esc closes it first.
- [x] Rescan works from the banner.
- [x] Window resize relayouts the treemap.

## Recommendations and drawer

- [x] Nothing is selected after a scan (S-09).
- [x] Row and item checkboxes select ready items only; blocked items show their reason and cannot be checked.
- [x] Select safe selects every ready safe-tier item.
- [x] The chevron opens the drawer with description, method, items and largest subfolders; Esc closes it.
- [x] Reveal in Finder opens Finder at the item.

## Copy script

- [x] With nothing selected, Copy script and Clean selected are dimmed and show "Select items to clean first".
- [x] Select npm cache and trash-bin, Copy script: a toast confirms, and the clipboard holds a script starting with `#!/bin/bash` and `set -euo pipefail`, a header with version, date, total and count, and one `# Item …` step per item.
- [x] Paths with spaces (`iOS DeviceSupport`) are single-quoted in the script.

## Cleanup, dry run

- [x] Select every ready item and press Clean selected: the dialog groups actions by row with method and size, lists items left out with the reason, and shows the dry-run note.
- [x] The confirm button reads "Clean <total>"; Enter does nothing, only a click starts it.
- [x] With a review-tier trash item selected (Hugging Face cache), "Delete permanently instead of moving to Trash" appears, unchecked; checking it removes the Trash note. With only iOS backups (allow_permanent = false) it does not appear.
- [x] Progress counts items; the results say "Dry run finished", "Would free …" and "Would move to Trash …".
- [x] Done rescans; every item is still there; `.sandbox/logs/cleanup.log` has one `dry_run` line per item.
- [x] Esc closes the confirm dialog and the results, but not the progress view.

## Cleanup, real (sandbox only)

- [x] **real** Clean every ready item: results show freed space, space moved to Trash, and no skipped items.
- [ ] **real** Trashed items (Android SDK, Hugging Face hub, LM Studio models, Xcode archives contents, iOS backup) are in the macOS Trash; Finder's Put Back works. (2026-10-25: items reach the Trash, but Put Back is not offered; see Results.)
- [x] **real** After Done, the rescan shows only blocked rows (cargo and uv while those processes run, and the dirty and detached worktrees); `Library/Keychains`, `.ssh`, and `.sandbox/outside` are untouched.
- [x] **real** Open a terminal in `.sandbox/home/.npm/_cacache` and replace it with a symlink after planning (or touch a file in a quiet-guarded folder): that item is skipped as "Changed since scan" and the rest continue.
- [x] **real** The audit log has a line per item with timestamp, rule, path, method, bytes and result.

## Relaunch

- [ ] Quit and relaunch: the last snapshot loads with "Scanned X ago", and selection is empty. (2026-10-25: snapshot loads and selection is empty, but "Scanned X ago" is not rendered; see Results. Built in Phase 5; recheck.)

## Phase 5: access, appearance, keyboard

- [ ] Fresh macOS user account, DiskLens not yet granted Full Disk Access: the first launch shows "Allow Full Disk Access". Open System Settings lands on Privacy & Security, Full Disk Access.
- [ ] Grant access, switch back to DiskLens: onboarding closes without a restart.
- [ ] Revoke access and relaunch. Continue without access, then quit and relaunch: onboarding does not show again (`settings.json` in the data dir has `"onboarding_done": true`).
- [ ] Without access, scan the real home folder (read-only): the banner counts unreadable folders and its Grant Full Disk Access button opens the same pane.
- [ ] Hover a tile that no rule matches: the tooltip shows its category's description. A matched tile shows its rule's description.
- [ ] Switch macOS between Light and Dark while the app runs: every surface repaints, with no light leftovers in dark mode (dialogs, tooltips and the toast included).
- [ ] Tab from the window start: focus moves through header controls, tiles, row checkboxes, chevrons and footer buttons with a visible ring; Shift-Tab goes back. Mouse clicks show no ring.
- [ ] Enter on a focused folder tile drills in, and Esc still goes up afterwards. Space on a row checkbox toggles it. Enter on a chevron opens the drawer.
- [ ] In the cleanup confirm dialog, Tab never reaches the Clean button and Enter does nothing; Cancel is reachable.
- [ ] VoiceOver (Cmd-F5): tiles read as name, size and category; checkboxes read their label and checked state; buttons read their names.

## Phase 6: packaging and updates

Build with `scripts/package.sh` (add the minisign key variables for the update artifacts). The app is `target/packager/DiskLens.app`.

- [ ] `lipo -archs target/packager/DiskLens.app/Contents/MacOS/disklens` prints `x86_64 arm64`; `codesign -dv` shows `com.madushan.disklens` and `flags=0x10002(adhoc,runtime)`; `Info.plist` has version 1.0.0 and `LSMinimumSystemVersion` 13.0; the DMG is under 25 MB.
- [ ] Open the DMG: it shows DiskLens and an Applications link. Copy the app, launch it: the new icon shows in Finder and the Dock, and the app runs under hardened runtime (onboarding or the last scan appears).
- [ ] System Settings, Full Disk Access lists "DiskLens"; granting it closes onboarding.
- [ ] Updates, no release: launch with `DISKLENS_UPDATE_URL=http://127.0.0.1:8000/none.json` (nothing served): no banner, a warning in the log.
- [ ] Updates, round trip: package 1.0.0 and keep its app; bump to 1.0.1 and package again with the key; serve `target/packager` with `python3 -m http.server 8000`; run the 1.0.0 app with `DISKLENS_UPDATE_URL=http://127.0.0.1:8000/latest.json`. The banner offers 1.0.1; Install, then Restart; the relaunched app is 1.0.1.
- [ ] Updates, tampered signature: edit one character of the signature in the served `latest.json`; Install fails with a message and the app stays 1.0.0.
- [ ] During a scan the banner does not offer Install.
- [ ] Clean macOS 13 machine (or VM): the app launches.

## Phase 7: background rescan

- [x] Open a subfolder, then Rescan: the explorer, stat cards and recommendations stay on screen; the header reads "Updating… N files" with Cancel; afterwards the same subfolder is shown and the header reads "Scanned just now".
- [x] Select a few items, open a drawer, then Rescan: during the scan Clean selected is dimmed with the tooltip "Available when the update finishes", Copy script still works; afterwards the same items are selected and the drawer is still open.
- [x] Delete a folder you are viewing in the sandbox from the terminal (for example `rm -rf .sandbox/home/<folder>`), then Rescan: the explorer falls back to its parent.
- [x] Select an item, delete its folder from the terminal, then Rescan: a toast says "1 selected item no longer found".
- [x] Rescan, then Cancel: results stay; the header shows "Scan cancelled" with Retry, and Retry starts a new scan.
- [x] **real** Clean a few safe items: as each finishes, its row leaves the panel and the totals and tiles shrink, before the follow-up rescan finishes.
- [x] Stale snapshot: set the system clock forward more than 24 hours (or wait), relaunch: the last results show at once and a background rescan starts; a fresh snapshot does not rescan on launch.

Results (2026-10-31, macOS, sandbox): all seven checks passed, run by the user in the native window.

## Phase 8: design foundation and window shell

Run with `DISKLENS_MOCK=preselect cargo run`, in light and in dark appearance.

- [ ] Side by side with `docs/mockup-v2.png` (light) and `docs/mockup-v2-dark.png` (dark): sidebar (blurred, traffic lights inside it, pages, six categories by size with folder counts), toolbar order (sidebar toggle, back and forward on Explore, title or breadcrumb, status with Rescan, inspector, queue, Clean up), fonts and colors. List the differences in PLAN.md (Phase 8b).
- [ ] Window opens at 1280x820 and cannot shrink below 960x640; the window drags by the toolbar.
- [ ] Clean up reads "Clean up 52.5 GB" in white on green; Clear Queue (Edit menu) turns it into a disabled "Clean up". During Rescan it is disabled with the tooltip "Available when the update finishes".
- [ ] Keyboard: Tab reaches the sidebar pages, the toolbar buttons and the page content. ⌘1, ⌘2, ⌘3 switch pages; ⌥⌘I toggles the inspector; ⌃⌘S the sidebar; ⌘R rescans; ⌘⇧R reveals the open folder; ⌘⇧H switches the Explore view mode. Command Palette, Show Queue, Quick Look, Export Cleanup Script and Keyboard Shortcuts are greyed out in the menus.
- [ ] Explore: open a folder, Back goes up, Forward reopens it; opening another folder disables Forward.
- [ ] VoiceOver (⌘F5): toolbar buttons read "Toggle Sidebar", "Back", "Forward", "Rescan", "Inspector", "Queue" and "Clean up …"; sidebar pages read their names.
- [ ] Reduce Transparency on (System Settings > Accessibility > Display), then focus the window: the sidebar turns opaque.
- [ ] Sandbox run (`DISKLENS_SCAN_ROOT=$PWD/.sandbox/home cargo run`) without Full Disk Access: the sidebar footer shows "Full Disk Access is off" with Grant Access…, and no banner sits above the page.

## Phase 9: Overview

Run with `DISKLENS_MOCK=preselect cargo run`, in light and in dark appearance.

- [x] Side by side with frame 1 of `docs/mockup-v2.png` and `docs/mockup-v2-dark.png`: Disk (Macintosh HD, 39.5 GB available of 463 GB, +52.5 GB after cleanup, capacity bar and legend, the 4.2 GB purgeable line), Safe to clean (41.7 GB, 8 items), Worth a look (56.0 GB, 10 items), Largest folders (Desktop, Library, .ollama, .npm, .android). List the differences in PLAN.md (Phase 9).
- [ ] Clear Queue (Edit menu), then Select Safe Items: the toolbar reads "Clean up 41.7 GB" and the "after cleanup" line reads +41.7 GB.
- [ ] Review opens Recommended. Clicking Desktop opens Explore at `~/Desktop`.
- [ ] VoiceOver (⌘F5): each widget reads as one sentence, and the folder rows read "Desktop, 41.5 GB, open in Explore".
- [ ] Does the lower half of the window read as empty at 1280×820?
- [ ] Sandbox run (`DISKLENS_SCAN_ROOT=$PWD/.sandbox/home cargo run`): the Disk widget shows the real volume name (after a rescan, if the snapshot predates Phase 9); the purgeable line is hidden below 1 GB.

## Phase 10: Explore and inspector

Run with `DISKLENS_MOCK=preselect cargo run`, in light and in dark appearance, then on the sandbox.

- [x] Side by side with frame 2 (Explore with Desktop selected) of `docs/mockup-v2.png` and `docs/mockup-v2-dark.png`: 12 tiles plus "3 smaller items, 9.8 GB", textures (.npm striped, .ollama faded), cleanable lines, the texture key, and the inspector (Size, Files 612,480, Category, Cleanable 27.9 GB in 4 items, Largest folders, Recommended inside). List the differences in PLAN.md (Phase 10).
- [x] Highlight Cleanable (⌘⇧H) dims tiles with nothing to clean and the key reads "Highlight Cleanable is on" (compare frame 8).
- [x] Click a tile, then use h/j/k/l and the arrows: the selection moves to the nearest tile and stops at the edges. Space stages and unstages (the toolbar queue count changes); Space on Documents shows "Nothing here can be cleaned".
- [x] Return or double-click on Desktop zooms into it from the tile; Backspace, Esc or ⌘↑ shrinks back into the Desktop tile and leaves it selected. With Reduce Motion on, both cross-fade.
- [x] Right-click a tile: Add to Queue (when something inside is cleanable), Reveal in Finder, Quick Look, Show Recommendation (on a matched tile), Get Info.
- [x] Sandbox run (`DISKLENS_SCAN_ROOT=$PWD/.sandbox/home cargo run`): ⌘Y on a file or folder opens Quick Look and ⌘Y again closes it; `~/Library` shows its system folders gray with a lock and the tooltip "Managed by macOS or an app".
- [x] VoiceOver (⌘F5): tiles read "name, size, category", and system tiles read "Managed by macOS or an app".
- [x] Real home scan at the minimum window size (960×640): no clipped or overlapping text in the treemap. Save screenshots of Home and two deeper folders here. (2026-10-31: the user reported no clipping; screenshots not saved yet.)

## Phase 11: Recommended, staging queue and cleanup sheet

Run with `DISKLENS_MOCK=preselect cargo run`, in light and in dark appearance, then on the sandbox.

- [x] Side by side with frame 3 (Recommended with the inspector) of `docs/mockup-v2.png` and `docs/mockup-v2-dark.png`: tier pills and totals, 52 pt rows, the selected row, blocked reasons, sizes, chevrons, the fade at the bottom of the list, the footer "52.5 GB staged · 5 items" with Select Safe Items, and the inspector (pill, label, What it is, What happens if removed, Method, Items, Largest subfolders, Reveal in Finder). Badges are left for Phase 14. List the differences in PLAN.md (Phase 11). (2026-10-31: reported matching in light; dark not confirmed yet.)
- [x] Click a row: it highlights and fills the inspector; clicking its checkbox only stages it. Then j/k and the arrows move between rows across both tiers and scroll the row into view at the bottom; Space stages and unstages (Space on a blocked row shows "Nothing here can be cleaned"); Return opens the inspector after hiding it with ⌥⌘I; Esc clears the row. (2026-10-31: reported working.)
- [ ] Explore: right-click a matched tile, Show Recommendation opens Recommended with that row selected; the same from the inspector's "Recommended inside".
- [x] Side by side with frame 4: the toolbar queue button opens the queue on Overview, Explore and Recommended, grouped by row with method and sizes; the x removes one item; Clear Queue empties it; Esc and the button close it. View › Show Queue does the same. (2026-10-31: reported working.)
- [x] Export: File › Export Cleanup Script… (⌘E), the queue's Export… and the row menu's Export Cleanup Script… open the save panel in Downloads; Cancel writes nothing. Copy as Shell Script, then `pbpaste | diff - <saved file>` prints nothing. (2026-10-31: reported working.)
- [x] Right-click a row: Add to Queue or Remove from Queue (not on a blocked row), Reveal in Finder, Export Cleanup Script…, Copy to Clipboard (the last two disabled with an empty queue). Esc and a click outside close it. (2026-10-31: every item works with the mock except Reveal in Finder, which the mock never performs because its paths are not on disk; on the sandbox it reveals the item, confirmed by the user.)
- [x] Side by side with frame 6: Clean up opens the sheet under the toolbar over a dimmed window: "Clean up N items?", one line per row, the note, the permanent checkbox (with a review-tier Trash row staged), Copy as Shell Script, Cancel, and the green "Clean 52.5 GB". It slides down and back up along the same path; with Reduce Motion on it cross-fades. (2026-10-31: the sheet stopped halfway on its way up and vanished, because it slid a fixed 480 pt; it now slides its measured height, and the user confirmed the exit. Dark not confirmed yet.)
- [x] Tab through the sheet and press Enter on every stop: Enter never starts the cleanup (Cancel closes it). (2026-10-31: reported working; also covered by `enter_never_confirms_the_cleanup`.)
- [x] Sandbox run (`DISKLENS_SCAN_ROOT=$PWD/.sandbox/home cargo run`, dry run): the results show "Updating results…" until the rescan lands; Done closes the sheet and empties the queue. (2026-10-31: confirmed by the user.)
- [ ] Held line (needs a machine with local Time Machine snapshots: `tmutil listlocalsnapshots /`): after a real cleanup that frees at least 1 GB, the results say "X is still held by N local Time Machine snapshots." when free space grew by less than was freed. Only with the user present.
- [ ] VoiceOver (⌘F5): rows read "label, size", the queue's remove buttons read "Remove … from the queue", and the sheet's Clean button is not a Tab stop.

## Supervised real run (only when the user says so)

- [x] On the real home folder, select only the npm cache, note its size, run a real cleanup, and check that `du -sk ~/.npm` before and after differ by the freed size within 1%.

## Results (2026-10-25, macOS)

Run against `.sandbox` on macOS, plus the supervised `~/.npm` run. Gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (268 tests) pass.

- Cancel during a scan was checked on the real home folder (the sandbox scan finishes too fast to cancel).
- Real sandbox run: 31 items (22 deleted, 7 trashed, 2 commands ran), none skipped; the 29 files under `Library/Keychains`, `.ssh` and `.sandbox/outside` hashed the same before and after. Remaining rows: Cargo and uv (processes running) and the atlas worktrees.
- Symlink swap: `_cacache` skipped as "Changed since scan: the folder is a symlink", the symlink target untouched, the other 8 items deleted.
- Supervised `~/.npm` run: freed 3,878,219,776 bytes (`_cacache` and `_npx`); `du -sk ~/.npm` went from 3,814,780 to 27,456 KB, the same 3,787,324 KB.

Failures and findings:
- Put Back is not offered for trashed items. `MacTrash` uses the `trash` crate's `NsFileManager` method, which its docs say does not support Put Back (the Finder method does, but triggers an automation prompt and a sound). Decide: switch methods, or drop Put Back from this checklist.
- "Scanned X ago" is not rendered anywhere in `src/ui`; `docs/PLAN.md` listed it as done. It belongs to Phase 5 (PLAN section 5, Phase 5, item 2).
- Toggling "Delete permanently instead of moving to Trash" rebuilds the plan through `Cleanup::Planning`, so the confirm dialog visibly re-renders (`AppModel::set_permanent`). The result is correct.
