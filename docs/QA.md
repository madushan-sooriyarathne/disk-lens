# DiskLens: Manual QA checklist

Run on macOS against the fixture sandbox unless a step says otherwise. Rebuild it first with `scripts/make-fixture.sh`. Debug builds are dry runs by default (`DISKLENS_DRY_RUN=1`); steps marked **real** set `DISKLENS_DRY_RUN=0`.

```bash
DISKLENS_SCAN_ROOT=$PWD/.sandbox/home cargo run                      # dry run
DISKLENS_SCAN_ROOT=$PWD/.sandbox/home DISKLENS_DRY_RUN=0 cargo run   # real, sandbox only
DISKLENS_MOCK=preselect cargo run                                    # mock data
```

The audit log of sandboxed runs is `.sandbox/logs/cleanup.log`; of real-home runs `~/Library/Logs/DiskLens/cleanup.log`.

## Scan and explorer

- [ ] First launch with no snapshot shows the idle screen; Scan starts the scan and the progress screen animates.
- [ ] Cancel during a scan returns to the previous state with a notice.
- [ ] The treemap shows the sandbox; click drills down, the breadcrumb goes back, Esc goes up one folder.
- [ ] Category / Cleanup toggle recolors tiles; matched tiles are hatched.
- [ ] Hover shows the tooltip; right-click opens the context menu; Esc closes it first.
- [ ] Rescan works from the banner.
- [ ] Window resize relayouts the treemap.

## Recommendations and drawer

- [ ] Nothing is selected after a scan (S-09).
- [ ] Row and item checkboxes select ready items only; blocked items show their reason and cannot be checked.
- [ ] Select safe selects every ready safe-tier item.
- [ ] The chevron opens the drawer with description, method, items and largest subfolders; Esc closes it.
- [ ] Reveal in Finder opens Finder at the item.

## Copy script

- [ ] With nothing selected, Copy script and Clean selected are dimmed and show "Select items to clean first".
- [ ] Select npm cache and trash-bin, Copy script: a toast confirms, and the clipboard holds a script starting with `#!/bin/bash` and `set -euo pipefail`, a header with version, date, total and count, and one `# Item …` step per item.
- [ ] Paths with spaces (`iOS DeviceSupport`) are single-quoted in the script.

## Cleanup, dry run

- [ ] Select every ready item and press Clean selected: the dialog groups actions by row with method and size, lists items left out with the reason, and shows the dry-run note.
- [ ] The confirm button reads "Clean <total>"; Enter does nothing, only a click starts it.
- [ ] With a review-tier trash item selected (Hugging Face cache), "Delete permanently instead of moving to Trash" appears, unchecked; checking it removes the Trash note. With only iOS backups (allow_permanent = false) it does not appear.
- [ ] Progress counts items; the results say "Dry run finished", "Would free …" and "Would move to Trash …".
- [ ] Done rescans; every item is still there; `.sandbox/logs/cleanup.log` has one `dry_run` line per item.
- [ ] Esc closes the confirm dialog and the results, but not the progress view.

## Cleanup, real (sandbox only)

- [ ] **real** Clean every ready item: results show freed space, space moved to Trash, and no skipped items.
- [ ] **real** Trashed items (Android SDK, Hugging Face hub, LM Studio models, Xcode archives contents, iOS backup) are in the macOS Trash; Finder's Put Back works.
- [ ] **real** After Done, the rescan shows only blocked rows (cargo and uv while those processes run, and the dirty and detached worktrees); `Library/Keychains`, `.ssh`, and `.sandbox/outside` are untouched.
- [ ] **real** Open a terminal in `.sandbox/home/.npm/_cacache` and replace it with a symlink after planning (or touch a file in a quiet-guarded folder): that item is skipped as "Changed since scan" and the rest continue.
- [ ] **real** The audit log has a line per item with timestamp, rule, path, method, bytes and result.

## Relaunch

- [ ] Quit and relaunch: the last snapshot loads with "Scanned X ago", and selection is empty.

## Supervised real run (only when the user says so)

- [ ] On the real home folder, select only the npm cache, note its size, run a real cleanup, and check that `du -sk ~/.npm` before and after differ by the freed size within 1%.
