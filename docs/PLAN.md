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
| 7 Background rescan | Done, merged | v2, PRD section 13.4.2; `docs/QA.md` Phase 7 checks passed on 2026-10-31 |
| 8 Design foundation and window shell | Merged (8a and 8b) | v2, PRD sections 13.4.3 and 13.5. Split in two: 8a (GPUI 0.3.10, gpui-component evaluation, `docs/mockup-v2.png`) merged; 8b (tokens, fonts, chrome, shell, menus, motion, icons) merged |
| 9 Overview | Merged | v2, PRD section 13.4.4 |
| 10 Explore and inspector | Merged | v2, PRD sections 13.4.5 and 13.4.7 |
| 11 Recommended and cleanup sheet | Merged | v2, PRD sections 13.4.6, 13.4.8 and 13.4.10 |
| 12 Onboarding | Merged, awaiting QA and the spike | v2, PRD section 13.4.1 |
| 13 Command palette | Merged | v2, PRD section 13.4.9 (free part) |
| 14 Active-work guard and project artifacts | Built, awaiting review | v2, PRD section 13.4.11 (free). Branch `feat/phase-14-active-work` |
| 15 Pro licensing and gating | Merged | v2, PRD section 13.6. Split: 15a app side merged (manual checks in `docs/QA.md` Phase 15a passed on 2026-11-01); 15b website (`site/`) and license server (`server/`) merged, manual checks in `docs/QA.md` Phase 15b still to run. Decisions recorded in PRD section 13.8 on 2026-11-01 |
| 16 Lockfile tags and orphaned stores | Built, awaiting review | Pro, PRD section 13.10.1. Branch `feat/phase-16-lockfile-stores`. Pro store rules replace the free rows while a license is active (decided 2026-11-06). Manual checks in `docs/QA.md` Phase 16 |
| 17 APFS snapshot inspector | Not started | Pro, PRD section 13.10.5 |
| 18 Containers and VMs | Not started | Pro, PRD section 13.10.3 |
| 19 AI models hub | Not started | Pro, PRD section 13.10.2 |
| 20 Apple, mobile and creative scanners | Not started | Pro, PRD section 13.10.4 |
| 21 Menu bar agent | Not started | Pro, PRD section 13.10.6 |
| 22 Auto-sweep | Not started | Pro, PRD section 13.10.7; needs Phase 21 |
| 23+ Remaining Pro features | Not started | Other volumes, history, large and old files, duplicates, local rules file, clone-aware sizes; one phase each |

Last full gate run on macOS (Phase 14 merged with Phase 13, 2026-11-01): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` (569 tests, 3 release-only tests ignored) all pass.

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
- Decisions (2026-10-25): bundle identifier `com.madushan.disklens`; ad-hoc signing until a Developer ID certificate exists; signed auto-updates are part of v1; the icon is `assets/icons/icon.svg` (glassy treemap with a lens). Replaced on 2026-11-01 by the brand kit's glass icon: its rendered `icon.icns` (dark, also the bundle icon) and `icon-light.icns`, which hold the small cut in the 16 and 32 px slots. The kit's renders are used as they are because qlmanage, the only SVG renderer that ships with macOS, flattens the glass onto white.
- Built:
  - Version 1.0.0. `[package.metadata.packager]` in `Cargo.toml` (cargo-packager 0.11.8), the x86_64 target in `rust-toolchain.toml`, and `MACOSX_DEPLOYMENT_TARGET = 13.0` in `.cargo/config.toml`.
  - `scripts/package.sh`: builds both slices, joins them with `lipo`, and runs `cargo packager` for `DiskLens.app` and `DiskLens_<version>_universal.dmg` in `target/packager`. Hardened runtime, ad-hoc unless `DISKLENS_SIGNING_IDENTITY` is set; cargo-packager notarizes and staples when the `APPLE_API_*` variables are set. With the minisign key set it also writes `DiskLens.app.tar.gz`, its `.sig` and `latest.json`, then prints the `gh release create --draft` command without running it.
  - `scripts/make-icon.sh` extracts `icon-256.png` and `icon-256-light.png` (drawn by the onboarding: the light icon in dark mode, the dark icon in light mode) from `icon.icns` and `icon-light.icns` with `iconutil`.
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

**Phase 8a: GPUI bump, gpui-component evaluation, mockup**
- Built:
  - GPUI `=0.3.10` (`gpui-pre`, `gpui-pre-platform`, test-support). No source changes; the lockfile moves only GPUI's own tree (wgpu and naga 30, taffy 0.15); `accesskit_macos` is still pulled in. Mock and sandbox runs launch without errors, and the v1 UI renders as before in the native window (checked by the user in dark mode, 2026-10-31).
  - `docs/mockup-v2/mockup.html` with its token block (type scale, radii, spacing, system and status colors with AA ratios, the muted category palette, texture opacities), rendered by `scripts/render-mockup.sh` to `docs/mockup-v2.png` and `docs/mockup-v2-dark.png`. Eight frames: Overview, Explore with textures and the inspector, Recommended, queue, command palette, cleanup sheet, onboarding (Full Disk Access), Explore inside `~/Library` with Highlight Cleanable during a background rescan.
- gpui-component 0.8.1 evaluation (scratch spike, not committed; the dependency is added in 8b with its first use):
  - Theming: every color in the components checked (button, input, sidebar, menus, popover, tooltip, list, command, sheet, title bar, checkbox, switch) comes from `ThemeColor`; none is hardcoded. `Theme::global_mut` also takes the font family and size, both radii and shadows, so `theme.rs` can drive it entirely. Control heights come from `Size` (26, 30, 32, 40 pt by default, taller than macOS controls); `Size::Size(px)` sets HIG heights per control.
  - Root: `gpui_component::Root::new(view, window, cx)` must wrap the window's root view for its popovers, dialogs, sheets and notifications. `AppModel` stays the app entity under it; tests that render a component need the same wrapper. `init` also registers its inspector in debug builds.
  - Accessibility: button, checkbox, switch, input, sidebar menu, list and menus set AccessKit roles or labels; command, sheet, popover and tooltip set none.
  - Size: a release arm64 window with Root, input, sidebar, button and tooltip grows from 4.4 MB to 11.1 MB (+6.7 MB; `init` registers every component, so most of it links regardless). Expect roughly +13 MB on the universal binary; the DMG (9.0 MB today) should stay under the 25 MB target, to be measured in 8b.
  - Use: input, list, popup and context menus, popover, tooltip, button, checkbox. Hand-build: the sidebar (custom category rows), the toolbar and titlebar (native traffic lights through `TitlebarOptions`, not its `TitleBar`), the command palette (on its input and list, since `command` has no AccessKit roles) and the cleanup sheet (no roles; it must come down from the toolbar).
- Approved (2026-10-31): the mockup, as rendered at commit 715ee69, is the visual target for 8b onward.
- Mockup decisions: the Clean up button fill is `#1E7A34` (white text 5.4:1) because the PRD's system green with white text fails AA; tile size lines use `--tile-secondary`, which passes on every fill and stripe pixel; the `-dark` board is a second file next to `docs/mockup-v2.png`; the mock JSON is unchanged (numbers the v2 screens need beyond it are marked "not in mock" in the HTML).
- Open:
  - VoiceOver over gpui-component controls is checked in 8b once the shell uses them.
  - The Overview leaves the lower half of the window empty at the default size; the approved mockup keeps it that way, revisit in Phase 9 if it reads as unfinished in the app.

**Phase 8b: tokens, fonts, window chrome, shell, menus, motion, icons**
- Built:
  - `theme.rs` v2 tokens from the mockup, plus the type scale, radii, spacing and texture constants. `rules.toml` and RULES.md section 7 carry the muted category palette. One test per theme checks every text-on-fill pair for AA, including tile labels over the reduced fills and their darkest stripe pixel.
  - The system font with `tnum` set once on the window. GPUI resolves `.AppleSystemUIFontMonospaced` to SF Mono (a probe measured equal advances for every glyph), so paths use it and Geist, Geist Mono and `OFL.txt` are gone.
  - `get_category_totals` (`view.rs`, backend, source). A folder counts for a category when a rule matched it or it holds a project marker or equals a category path; the search stops 4 levels below the root to bound the lazy marker checks. Unclaimed bytes go to `documents_media` or `dev_tools` per top-level folder by media share, so the totals add up to the root size.
  - `chrome.rs`: 1280×820 default, 960×640 minimum, transparent titlebar with the traffic lights in the sidebar, blurred background on macOS.
  - gpui-component 0.8.1: `init`, `Root` around `AppModel`, and `theme::sync_components` writing its colors, fonts and radii from our tokens on startup and on every appearance change. Toolbar buttons and their tooltips are its `Button`.
  - `shell.rs`: sidebar (pages, categories by size as the legend, footer notice for Full Disk Access or unreadable folders, which replaces the banner), unified toolbar in FR-WIN-2 order and an empty 320 pt inspector slot. Overview, Explore and Recommended show the v1 views unchanged; the explorer's scan status and breadcrumb moved to the toolbar.
  - Menu bar (FR-WIN-4) with its shortcuts in `root.rs`.
  - `motion.rs`: the 0.35 s and 0.25 s critically damped springs, an easing that never overshoots, and the 150 ms cross-fade under Reduce Motion. `platform` reads Reduce Motion and Reduce Transparency; the sidebar turns opaque under the latter.
  - Fourteen line icons in `assets/icons/ui/` from the mockup, served by `assets.rs`.
  - Size with gpui-component: the universal DMG is 13.5 MB (9.0 MB before), the universal binary 26.8 MB; the 25 MB DMG target holds.
- Decisions:
  - The light sidebar's secondary text is `#636366`: the mockup's `#6E6E73` measures 4.2:1 on the sidebar fill.
  - Back goes up one folder and Forward reopens folders Back left, so Esc, Backspace and the toolbar agree; opening a folder or a new snapshot clears Forward.
  - Until Phase 11, the queue is the v1 selection: the queue button shows its count and size and toggles the inspector; Add to Queue selects the focused row's ready items.
  - Highlight Cleanable (⌘⇧H) switches the v1 view mode until Phase 10 removes `ViewMode`.
  - Export Cleanup Script, Command Palette, Show Queue, Quick Look and Keyboard Shortcuts are listed disabled until their phases.
  - The update banner stays above the page content.
  - The filter token slot is not drawn yet (Phase 13).
- Open:
  - Approved (2026-10-31): the user compared the running app with the mockup and reported no differences, and the sandbox run (sidebar footer, category totals, dry-run Clean up) passed.
  - VoiceOver: the user confirmed it reads the Overview button; the remaining sidebar and toolbar buttons are not confirmed yet (`docs/QA.md`, Phase 8).
  - One gate run had four `tests/scanner.rs` failures (sizes read as 0 right after the fixture files were written) that did not reproduce alone or in a second full run. Likely delayed block allocation under load; fix the test if it recurs.
  - Category folder counts and sizes in the sidebar depend on the depth bound; projects deeper than 4 levels fall back to `dev_tools` or `documents_media`.

**Phase 9: Overview**
- Built:
  - `VolumeInfo` records the localized volume name and the purgeable total (important-usage available minus `statfs` free). Both default when an older snapshot loads, so the Disk widget reads "Disk" until the next scan.
  - `overview.rs` replaces `stat_cards.rs`: Disk (free space, "+X after cleanup" with a selection, capacity bar of the six categories, System and apps, and Free, with its legend, and the purgeable line), Safe to clean with Select Safe Items, Worth a look with Review, and the five largest folders under the scan root, which open Explore at that folder (`AppModel::explore`).
  - `stats.rs`: tier counts are ready items (the mockup's 8 and 10), `capacity_segments` and `largest_folders`; `free_after_cleanup` is gone. Each widget carries a one-sentence accessibility label, unit-tested.
  - The mock gains `volume.name` "Macintosh HD" and 4.2 GB purgeable, the mockup's illustrative value.
- Decisions:
  - The purgeable line reads "Up to X of this is purgeable, mostly local Time Machine snapshots." The mockup's "Up to 4.2 GB more is held by…" would double count, because the available figure already includes purgeable space. The line shows from 1 GB.
  - Sizes of 100 GB and more keep the app's whole-number format, so the volume total reads "463 GB" where the mockup shows "462.5 GB".
  - `largest_folders` is loaded with each snapshot from the root view and stays as it is after a cleanup until the follow-up rescan, like the category totals.
- Visual check (2026-10-31, light and dark, against frame 1): approved. The one difference found, Select Safe Items and Review stretching across the widget, is fixed: they are label width and 28 pt high like the mockup's buttons. The other differences are the two decisions above.
- Open:
  - The rest of `docs/QA.md`, Phase 9: the selection and navigation clicks, VoiceOver, and the sandbox run.
  - Whether the lower half of the window still reads as empty at the default size (8a open item).

**Phase 10: Explore and inspector**
- Built:
  - `view.rs`: the view is one level, at most 12 children of 1% or more, then one smaller-items tile (`ViewKind::Smaller`, formerly `Other`) that lists what it merged. Nodes carry `file_count`, `cleanable` (bytes of recommended items at or below them) and `system_managed` (protected, or a `~/Library` folder with nothing cleanable). `merge_smaller` is shared with the mock. `ViewMode` is gone.
  - `treemap.rs`: flat squarified layout in the view's order with 4 pt gaps and flush outer edges; pure functions for the label rule (`tile_label`, `fit_middle`), keyboard neighbours (`neighbor`) and the zoom frame, with a property test that labels fit at the minimum window size.
  - `theme.rs`: `Texture` and `tile_fill`; the contrast test reads its surfaces from `tile_fill`.
  - `explorer.rs`: the Explore page with textures, the cleanable line, lock glyphs, the texture key, Highlight Cleanable, tooltips, the context menu (Add to Queue, Reveal in Finder, Quick Look, Show Recommendation, Get Info) and the drill-down zoom.
  - `inspector.rs`: folder, file and smaller-items panels. `keymap.rs`: h/j/k/l and the arrows, Space, Return, Backspace and ⌘↑ while the treemap has focus.
  - `platform/quick_look.rs`: `QLPreviewPanel` with a one-item data source.
- Decisions:
  - Quick Look calls run on the next run loop turn (`performSelectorOnMainThread`): showing the panel makes it key, and GPUI handles our window resigning key by borrowing the app, which the action handler already holds (it panicked when called directly). The panel opens without a responder-chain controller.
  - Texture follows the tile's tier: a tile at or inside a matched item is textured, a folder that only contains items stays solid with its cleanable line. A blocked safe item reads Restorable.
  - The smaller-items tile counts only the items it merged; leftover bytes alone read "Other files". Space on it stages only its children's items, never its folder's.
  - "Nothing here can be cleaned" is a toast rather than toolbar status text.
  - The inspector is open by default and shows on Explore only; Phase 11 adds Recommended. The inspector itself does not slide in.
  - Textured fills are opaque mixes, so Reduce Transparency leaves them as they are.
  - Mock cleanable sizes are clamped to the tile, because the mock's item sizes under `~/Library` exceed the folder.
- QA (2026-10-31): the user ran every Phase 10 check in `docs/QA.md` (mockup frames 2 and 8 in light and dark, Highlight Cleanable, the keymap, the zoom and its Reduce Motion fade, the context menu, Quick Look on the sandbox, VoiceOver, the real home scan at 960×640) and reported them all working, with no differences to list.
- Open:
  - Screenshots of the real home scan at the minimum window size are not saved in `docs/QA.md` yet (PRD V2-G2 asks for them).
  - Free disk space ran out once during this phase (`target/debug/incremental` had reached 4.3 GB); it was cleared, and the build cache will grow back.

**Phase 11: Recommended, staging queue and cleanup sheet**
- Built:
  - `recommendations.rs`: the Recommended page (tier pills and totals, 52 pt rows with a selected state, the bottom fade, the footer queue summary with Select Safe Items) and the row context menu. `keymap.rs` binds j/k, the arrows, Space and Return in the list's own context; `AppModel::move_row`, `stage_row` and `open_row` act on them.
  - `inspector.rs`: the recommendation panel (the v1 drawer content) and the queue panel. The drawer is gone. `AppModel::show_row` opens Recommended with a row; Show Recommendation and "Recommended inside" use it.
  - The queue: `selection::queue_groups`, `AppModel::queue_open` and `toggle_queue`, the toolbar queue button and View › Show Queue on every page. `shell::clean_button` is shared by the toolbar and the queue.
  - Export Cleanup Script… (⌘E, the queue, the row menu) through the save panel, and Copy as Shell Script, both from one builder (`queue_script`).
  - `cleanup.rs`: one sheet under the toolbar for confirm, progress and results, sliding down and back up on the `NAV` spring (cross-fade under Reduce Motion); `sheet_exit` keeps the closed sheet only while it slides away.
  - `platform::local_snapshot_count` (`/usr/bin/tmutil listlocalsnapshots /` through the Runner), `backend::free_space` (available minus purgeable) and `cleanup::held_by_snapshots` for the results line.
- Decisions (2026-10-31):
  - The snapshot count is pulled forward from Phase 17; the results line links nowhere until Phase 17 builds the inspector.
  - Badges wait for Phase 14, which adds their data.
  - The row menu's script covers the whole queue, like the File menu and the queue panel.
  - The queue opens in the inspector on every page, the Overview included.
  - The queue stays the selection: never saved (S-09) and carried over by `(rule_id, path)`.
  - The follow-up rescan starts when the cleanup finishes, so the results show "Updating results…"; Done only closes the sheet and clears the queue.
  - A single-item row gets one Reveal in Finder button in the inspector; a row with several items gets one Reveal per item.
  - The confirm view lists one line per row with its methods (a command shows its program); paths are in the queue panel and the script.
  - The mock's script uses the mock's scan time, so two exports of one plan are equal.
- Open:
  - QA (2026-10-31): the user ran every Phase 11 check in `docs/QA.md` (frames 3, 4 and 6 in light and dark, the row keys, the queue, export, the row menu with Reveal on the sandbox, Show Recommendation, the Enter rule, the sandbox dry run, VoiceOver) and reported them passing. One finding, fixed: the sheet stopped halfway on its way up and vanished (it slid a fixed 480 pt); it now slides its measured height.
  - The held line needs a machine with local snapshots and a real cleanup.
  - Switching to Recommended does not move keyboard focus into the list; click a row or Tab to it first.

**Phase 12: Onboarding**
- Built:
  - `onboarding.rs`: Welcome, Full Disk Access and First scan (`Phase::Onboarding(Step)`), centred on the window over the brand colors (Midnight base in dark mode, white in light, an Electric Blue glow from the top and a hint of Cantaloupe from the bottom) and film grain (`assets/onboarding/noise.png` from `scripts/make-noise.py`), with Back and the page dots at the bottom, on the edges of the content column. A dot goes to any step reached before, Esc goes back, and each step slides in from the side it comes from (a cross-fade under Reduce Motion). It shows when no last snapshot loads and `onboarding_done` is false.
  - `fda_preview.rs`: a drawn copy of the macOS 26 Full Disk Access pane (from the user's screenshot) with DiskLens switching on, in light or dark with the app. The other rows are neutral placeholders, so no other app's name or icon is shown. In a short window the pane is cropped below the DiskLens row.
  - The access step checks the probe at once and every 500 ms (`ACCESS_POLL`) while it is on screen, and on window activation; a grant shows a check mark and moves on after 400 ms. Leaving the step drops the poll.
  - Open System Settings: `platform::preseed_access` lists `~/.Trash` (one silent protected read), then the deep link. The window keeps its size and position.
  - Drag card: the app icon as an AppKit drag source carrying the bundle URL (`platform/drag.rs`; under `cargo run`, the binary), with Show in Finder as the fallback.
  - First scan: `category::Live` tallies bytes per category during the walk (path, marker in the folder, media extension); `ScanEvent::Progress` carries the totals. The step shows the bytes so far large, a stacked category bar and a legend. The exact totals replace them at finish.
  - Full Disk Access checks go through the `platform::Access` trait; `FakeAccess` records paths, and a test proves the onboarding probes never touch Desktop, Documents or Downloads.
  - `DISKLENS_MOCK=onboarding` starts the mock as a first launch; access is granted 6 s after Open System Settings.
- Decisions (2026-10-31):
  - Live category totals are rough while scanning (hard links count per link, loose media files count as media instead of the folder share); exact at finish.
  - Native drag plus the Finder fallback; the spike decides whether the native drag stays.
  - The spike runs on this Mac (macOS 26.6) with a fresh account; macOS 13, 14 and 15 are recorded as untested.
  - Onboarding shows only without a last snapshot, so an upgraded v1 user with a snapshot skips it. Continue Without no longer marks it done; the first finished scan does, from onboarding or the idle screen.
  - Cancel during the first scan goes to the idle screen; the next launch starts again at Welcome.
  - Redesign after the first review (2026-11-01): no card, content on the window like Raycast's onboarding; the animated toggle became the drawn System Settings pane; the drag icon is always shown.
  - Second review (2026-11-01): the app icon is a pre-rendered PNG (`icon-256.png`), decoded when the window opens, because GPUI rasterized the SVG on first draw. Moving the window beside System Settings was dropped at the user's request (it resized the window); `platform/window.rs` is gone. If a window frame change comes back, make it outside GPUI's update: AppKit resizes synchronously and GPUI's resize handler cannot borrow the app inside an update, which leaves part of the window undrawn. Back from the first scan cancels it and returns to Full Disk Access; the first scan cannot restart until the cancelled one stops. Only the incoming step animates.
  - Brand pass (2026-11-01): the app icon is the brand kit's glass icon; the bundle uses the dark `.icns` and the onboarding shows the light icon in dark mode and the dark icon in light mode. Film grain stays in the backdrop, under the content. The fade under the System Settings preview is one gradient ending in the composited backdrop color, and bands whose opacity follows it paint the same noise on the same window grid, so the grain the fade covers comes back. The first scan ends on Scan complete (safe to clean, worth a look, Go to Overview) instead of jumping to the Overview. Onboarding takes the brand colors: buttons in the deep brand blue `#0277B5` (white on Electric Blue is only 3.2:1; the AA test covers the pair), the active dot and first Welcome feature in Electric Blue, and the backdrop above. The Cantaloupe hint goes past the brand kit's orange-for-reclaimable rule, at the user's request. A real macOS 26 Liquid Glass icon (Icon Composer `.icon`, compiled by `actool`) is not done.
- Open:
  - The spike in `docs/QA.md` (Phase 12): whether `~/.Trash` adds DiskLens to the list without a prompt, and whether the list accepts the dragged app. Anything that shows a prompt is dropped.
  - The native drag starts from GPUI's mouse-down; whether GPUI's own pressed state recovers after the drag is part of the spike.

**Phase 13: Command palette**
- Built:
  - `palette.rs`: ⌘K (View › Command Palette) opens it over Overview, Explore and Recommended, not during onboarding, over the cleanup sheet or over a notice. ↑/↓ and ⌃N/⌃P move (bound in the `Palette` key context, `keymap.rs`), Return runs, Esc clears the field, then closes. An empty field shows four suggestions: the three pages and Rescan.
  - Look of Raycast's compact window: a floating panel with no scrim, a 52 pt search bar, 40 pt rows with a line icon, the title and on the right the shortcut, kind, folder size or Pro tag, and a footer naming what Return does. The field is gpui-component's `Input`; the list is drawn in `palette.rs`, because the kit's `Command` fixes a small field and plain rows.
  - Groups: Pages, Recommendations ("Show …" and, for rows with ready items, "Stage …"/"Unstage …"), Commands with their shortcuts, Pro, and Folders. Everything but folders is ranked by `view::Fuzzy` (nucleo-matcher, smart case); rows also match on their rule ID.
  - `view::find_folders`: fuzzy over folder names in the snapshot, optionally below one folder, top 50 by score then size, paths built only for those. Off the UI thread, late results dropped. A release-only test keeps 400k folders under 100 ms.
  - `filter.rs`: `size>`, `size<` (decimal units, bare numbers are bytes), `tier:`, `category:` (a `rules.toml` id) and `idle>Nd`, all of which must hold; errors name the bad term. `> filter …` shows one entry (or the error, disabled); running it sets the toolbar token, Explore dims tiles that don't match, Recommended hides rows that don't (and their share of the tier totals; j/k skips them).
  - `idle>Nd` reads `RecItem.idle_days`: `guards::project_age` (the `project_idle` evidence, now returning the age) for items of rules with `project_idle_days`. A row is as idle as its most idle item; a tile without an age never matches.
  - `/` on Explore opens the palette limited to folders under the open one.
  - Pro batch commands (stage a rule, stage by filter, saved filters, run a sweep rule) are listed with a Pro tag and open an explanation sheet until Phase 15. Help › Keyboard Shortcuts opens a sheet listing every shortcut.
  - A snapshot swap closes an open palette. No palette entry opens or confirms a cleanup (S-11, tested over every listed entry).
- Decisions (2026-11-01):
  - `idle>30d` works now instead of waiting for Phase 14: the analysis runs the project age check once more per idle-rule item (a second `git log`, once per snapshot, off the UI thread; marked `ponytail:` in `recommend/build.rs`).
  - Select Safe keeps its v1 meaning under a filter (every ready safe item), and the queue summary counts everything staged.
- Open:
  - The manual checks in `docs/QA.md` (Phase 13), including the open time on a real home snapshot.

**Phase 14: Active-work guard and project artifacts**
- Built:
  - `rules/git_state.rs` reads a repository in process with libgit2 (`git2`, vendored and static, no https or ssh): status entries (untracked included, untracked folders as one entry, ignored files excluded, the index never written) and, per local branch and a detached HEAD, the commits no `refs/remotes/*` ref contains (counted up to 1000).
  - `git_active`, the first *demoting* guard (RULES.md section 4.2). `recommend::build::analyze` checks it once per snapshot, reading each repository once. An affected item moves to the review row "<label> (active work)" (`<row_id>:active`) with `RecItem.active_work` set to the reason; the treemap draws it as review tier (`Analysis::tier_of`). It never blocks, so revalidation and cleanup are unchanged.
  - Rules: `rust-target-idle` and `python-venv-idle` (safe), `dist-idle` (review), all with `project_idle_days = 30`, `quiet_minutes = 60` and `git_active`; `node-modules-idle` and `next-turbo-build` get `git_active`. Idle evidence adds `Cargo.toml`, `Cargo.lock`, `pyproject.toml`, `requirements.txt`, `uv.lock` and `poetry.lock`. Every rule has a required, non-empty `keywords` list; the palette matches recommendation rows on their rule's ID and keywords, so "dormant rust targets" finds `rust-target-idle` (FR-KB-2).
  - UI: an amber Active work pill on the row and in the inspector, with each item's reason under its path; the mock marks one item.
  - Fixture v4: `Desktop/aw-clean`, `aw-dirty`, `aw-unpushed` and `aw-noupstream` (each with ignored `target` and `node_modules`), plus idle `py-proj` and `js-lib`; fixture commits are dated 2024-01-01.
- Decisions (2026-11-01):
  - Built off `main` while Phase 13 was in progress, then merged with it: `Item` and `RecItem` carry both `idle_days` (Phase 13) and `active_work`, and `guards::project_age` reads the extended evidence list.
  - Rust `target` and Python virtual environments are safe tier; `dist` is review (PRD FR-GIT-4).
  - Changes inside the item itself do not count (build output is often untracked). A repository with no remote counts as active, as FR-GIT-1 reads; so the fixture's `Desktop/gitrepo` `.next` moves to review. An item with no repository below the scan root is not demoted.
  - Checked once per snapshot, not on every refresh: the treemap needs the same tier, and status is too slow for window activation. A repository that changes after the scan is picked up by the next scan.
- Measured: `git_state::read` over the sandbox repositories takes 2 to 10 ms each (release; target 50 ms). git2 adds 0.80 MB to the arm64 release binary (12.86 to 13.66 MB). `scripts/package.sh` (ad-hoc): universal DMG 15.7 MB (target 25 MB), universal binary 29.4 MB.
- Open: the native-window check in `docs/QA.md` (Phase 14).

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
| UI | GPUI, Zed's GPU UI framework: `gpui-pre` and `gpui-pre-platform` pinned to an exact snapshot version (`=0.3.10`), imported as `gpui` and `gpui_platform`. v1: no component library. v2: `gpui-component` from gpui-kit (0.8.1, which pins `gpui-pre =0.3.10`, so GPUI is bumped first, in its own commit) for the palette, text input, lists, menus, popovers and tooltips, themed with DiskLens tokens. Any component that can't follow the HIG is hand-built |
| Styling | Theme tokens in `src/ui/theme.rs`; category colors come from `rules.toml` |
| State | One GPUI entity, `AppModel` (`src/ui/model.rs`) |
| Treemap | Squarified layout ported from d3 into `src/ui/treemap.rs`, rendered as absolutely positioned divs |
| Fonts | v1: Geist and Geist Mono static TTFs (OFL) embedded from `assets/fonts/`. v2 (Phase 8b): the system font (`.SystemUIFont`) with tabular figures (`tnum`, set once on the window); paths in `.AppleSystemUIFontMonospaced` (SF Mono). No font files are bundled |
| Motion (v2) | GPUI's `spring.rs` and `with_animation`; no extra crate |
| Tooling | cargo (`fmt`, `clippy`, `test`) |
| Packaging | `cargo-packager` CLI 0.11.8 (`.app`, DMG, codesign, notarization, minisign update signatures) |
| Web (Phase 15b) | Bun. `server/`: TypeScript, `Bun.serve`, `bun:sqlite`, `@payments-lk/node`, AWS SES (`@aws-sdk/client-sesv2`) with React Email. `site/`: Next.js App Router, React, MDX, plain CSS |

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
    make-icon.sh        assets/icons/icon-256.png and icon-256-light.png from the icns files
  rules/rules.toml
  assets/
    mock/snapshot.mock.json   matches the mockup numbers (DISKLENS_MOCK)
    icons/              icon.icns, icon-light.icns (sources, from the brand kit), icon-256*.png (generated), ui/*.svg (line icons)
    update.pub          minisign public key for updates
    license.pub         minisign public key for license tokens (Phase 15a)
  src/
    main.rs, lib.rs, backend.rs, state.rs, error.rs, config.rs, view.rs, settings.rs, update.rs
    scanner/  walk.rs, tree.rs, hardlinks.rs, volume.rs, snapshot.rs
    rules/    model.rs, load.rs, validate.rs, category.rs, matcher.rs, guards.rs, tools.rs
    providers/ ollama.rs, git_worktrees.rs, simctl.rs, docker.rs
    recommend/ build.rs, model.rs
    cleanup/  model.rs, protected.rs, plan.rs, revalidate.rs, execute.rs, script.rs, audit.rs
    platform/ mod.rs
    ui/       mod.rs, root.rs, model.rs, source.rs, theme.rs, format.rs, stats.rs,
              selection.rs, treemap.rs, screens.rs, overview.rs, explorer.rs,
              recommendations.rs, cleanup.rs, chrome.rs, shell.rs, motion.rs, assets.rs
    bin/scan-bench.rs
    v2 adds: ui/palette.rs, ui/keymap.rs, ui/queue.rs, ui/inspector.rs,
             rules/git_state.rs, providers/{store_orphans,containers,models,simruntimes}.rs,
             snapshots.rs (APFS), license.rs, sweep.rs, bin/disklens-agent.rs
  tests/      scanner.rs, rules.rs, rules_doc_sync.rs, sandbox_rows.rs, view.rs,
              cleanup_adversarial.rs, cleanup_engine.rs, cleanup_backend.rs, script.rs,
              support/mod.rs (temporary scan roots under target/tmp)
  server/     license server (Bun): src/{index,app,license,db,orders,trials,mail,limit}.ts,
              emails/LicenseEmail.tsx, test/ (fixtures/contract.json is shared with tests/license_contract.rs)
  site/       marketing site (Next.js): app/, components/, content/features.ts, public/
```

## 3. Data contracts

Rust types derive `Serialize` and `Deserialize` (snapshot persistence and the mock). The UI uses these types directly; there is no separate frontend contract.

```rust
pub struct VolumeInfo { pub total: u64, pub available: u64, pub name: String, pub purgeable: u64 }

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

// Phase 15: license token, shared by the app and the license server (server/)
// token   = "DL1." b64url(payload_json) "." b64url(minisig_text)   (base64url, no padding)
// payload = {"v":1,"id":"lic_…","kind":"pro"|"trial","issued":<unix s>,"max_major":<u32>,"expires":<unix s>|null}
// minisig_text: a minisign signature (prehashed ED) over the exact payload_json bytes, trusted comment = id,
// verified against assets/license.pub (not the update key). Valid when the signature verifies, v == 1,
// the app's major version <= max_major, expires is null or in the future, and a trial has expires.
pub enum LicenseStatus { Free, Pro { id: String }, Trial { id: String, ends: u64 },
                         Ended { kind: Kind, id: String }, Invalid(LicenseError) }  // Kind: Pro or Trial

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
| `remove_license` | none | none (Phase 15) |
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
2. Produce `docs/mockup-v2.png` and `docs/mockup-v2-dark.png` (light and dark: Overview, Explore with textures, Recommended, queue, command palette, cleanup sheet, onboarding) from the direction in PRD section 13.5, and get it approved before code. Update `assets/mock/snapshot.mock.json` to its numbers.
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
3. Open System Settings: the pre-seed read and the deep link. The window keeps its frame.
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

Decisions (2026-11-01, PRD section 13.8): one-time purchase with paid major upgrades, a 7-day trial issued as a signed token, payments through payments.lk, and our own license server that signs tokens. Split in two branches.

**15a: app side** (`feat/phase-15a-licensing`). The app makes no network call for licensing.

1. `src/license.rs`: the token in section 3, verified offline against `assets/license.pub` with `minisign-verify`; stored in `settings.json`. `license_status`, `activate_license` and `remove_license` in `backend.rs`. `scripts/mint-license.sh` mints tokens for development and QA from `DISKLENS_LICENSE_KEY_PATH`.
2. The `pro = true` rule field: the matcher skips those rules without a license (`Env.pro`). A license change starts a background rescan.
3. The Pro sheet: the feature's explanation, Buy DiskLens Pro (`<site>/buy`), Start 7-Day Trial (`<site>/trial`) and Enter License…. Pro status in the sidebar footer. "More with Pro" at the end of Recommended while any `pro = true` rule exists.
4. A Settings window (⌘,) with the license. `disklens://activate?token=…` opens it with the token filled in; activating always takes a click. Closing it does not quit the app.
5. Palette batch commands (stage a rule, stage by filter) and saved filters (PRD FR-KB-4), all of which only stage. Run Sweep Rule Now stays an explanation until Phase 22.

**15b: website and license server** (`feat/phase-15b-web`). Two separate deployables beside the crate, each with its own `package.json` and lockfile; no JS or Cargo workspace. They share only the token contract (section 3) and HTTP.

1. `server/` (Bun and TypeScript, `Bun.serve` routes, `bun:sqlite`), the only code that holds secrets and talks to payments.lk:
   - `GET /health` (`ok`, mode, signing key ID) and `GET /price` (`PRO_PRICE_CENTS`, LKR).
   - `POST /checkout {email}`: a pending order with a random `ord_` ref, then a payments.lk checkout (`reference` and idempotency key = ref, success URL `<site>/success?ref=…`); returns the checkout URL.
   - `POST /webhooks/payments-lk`: verified with the SDK's `constructEvent` (`payments-signature`, HMAC-SHA256, 300 s tolerance). Events whose mode differs from the server's key (test or live) are ignored.
   - `GET /orders/:ref`: the order status, and the token once paid. A pending order asks payments.lk for its checkout once, so a missing or late webhook still settles.
   - Settling is shared by the webhook and the poll and happens once per order: the payment must have succeeded, be in LKR, match the mode and cover the order's amount. Only the call that mints sends the email.
   - `POST /trial {email}` and `POST /recover {email}`: always answer 202. A trial is minted once per normalized email and re-sent on repeat. Rate limited per email and per IP.
   - Tokens are signed with `node:crypto` (Ed25519, BLAKE2b) in the minisign layout from the license secret key (`LICENSE_SECRET_KEY`, unencrypted). Email goes through AWS SES with React Email; `MAIL_DRY_RUN=1` writes it to `server/.mail/` instead.
2. `site/` (Next.js App Router on Bun, plain CSS, no secrets; only `NEXT_PUBLIC_*` env): landing, features, pricing (price from the server), download, docs, FAQ, changelog, privacy, terms, and the buy, trial, success, activate (`#<token>`), recover and cancel pages. Pro features come from `site/content/features.ts` with an available or coming status.
3. Contract: `server/test/fixtures/contract.json` holds a token minted by the server code with a test-only key, and `tests/license_contract.rs` verifies it with the app's code.
4. The app's default site becomes `https://disklens.space`.

**Acceptance (15b):**
- `bun test` (server) covers: checkout; webhook signature, replay and mode checks; underpayment; poll settling; one trial per email; recovery; CORS. No network calls in tests.
- `cargo test --test license_contract` passes; `bun run build` (site) passes.
- Manual checks in `docs/QA.md` Phase 15b: a sandbox purchase activates Pro in the packaged app, a trial arrives only by email, and recovery re-sends both.

**Acceptance (15a):**
- Tests: a valid token activates Pro; a tampered, ended, wrong-key or older-major token does not; no network call is made.
- A test proves that scanning, recommendations of free rules, guards, plan building, execution and script export behave the same with and without a license.
- A test that a `pro = true` rule produces no rows without a license and does with one.

### Phase 16: Lockfile tags and orphaned stores

Decision (2026-11-06): the Pro store rules (`pnpm-store-orphans`, `bun-cache-orphans`, `cargo-registry-orphans`, `homebrew-old`) claim the same folders as the free store rules with `priority = 1`, so they replace the free rows only while Pro is active. The matcher keeps one rule per path and revalidation is unchanged; Pro users no longer get the whole-store rows for these four stores. The snapshot did not keep link counts, so it gained `single_link_bytes` per folder and the list of lockfiles seen by the scan.

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
