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
