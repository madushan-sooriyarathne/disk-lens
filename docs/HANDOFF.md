# DiskLens handoff prompt

Paste everything below the line into a new Claude Code session opened in `/Users/madushan/Developer/disk-lense`.

---

You are continuing work on DiskLens, a macOS disk space analyzer and safe cleaner. It is one Rust crate at the repo root: the backend plus a native UI drawn with GPUI (Zed's GPU UI framework) in `src/ui/`.

## Start here

1. Read `CLAUDE.md`, then `docs/PRD.md`, `docs/RULES.md`, `docs/PLAN.md` in full. They are the source of truth. If they conflict or something is unclear, ask me instead of guessing. `docs/mockup.jpg` is the target UI.
2. Run `git status` and `git log --oneline` to confirm state. Work is on branch `gpui-migration` (nothing pushed). The tree should be clean.
3. Read "Status and roadmap" at the top of `docs/PLAN.md` for what is left.

## How we work (unchanged)

- Follow `docs/PLAN.md` one phase at a time. Before writing code for a phase, enter plan mode, propose the task breakdown, and wait for my approval.
- Commit after each task with conventional commit messages (end them with the Co-Authored-By line your environment specifies).
- At the end of each phase run its acceptance checks, give me a short summary of what was built, what passed, and what you could not verify, then stop for my review.
- Use the latest stable dependency versions and check official docs and crate sources instead of relying on memory. GPUI is pinned to the `gpui-pre` `=0.3.9` snapshot; read its source and examples in `~/.cargo/registry/src/*/gpui-pre-0.3.9/`.
- Replies are terse (caveman style) but commits, code and docs are normal prose.

## Safety rules (non-negotiable)

- Never delete, trash or run cleanup against my real home folder. Develop against the fixture sandbox: `DISKLENS_SCAN_ROOT=$PWD/.sandbox/home cargo run`. Rebuild the sandbox with `scripts/make-fixture.sh`.
- Debug builds default to `DISKLENS_DRY_RUN=1` (`src/config.rs`).
- Do not run `rm`, `git clean` or any cleanup command outside this repository.
- Real cleanup execution is enabled only in Phase 4, on one safe item (the npm cache), and only when I explicitly say so.
- Read-only scans of real folders with `scan-bench` are fine (they write nothing).
- `rules/rules.toml` must stay verbatim equal to the TOML block in `docs/RULES.md` section 7. Change both, plus tests, in the same commit (`tests/rules_doc_sync.rs` enforces it).

## What is done

- **Phases 0 to 3** (scaffold, scanner, explorer UI, rules engine and recommendations) were built on Tauri and React. The scanner is within 1% of `du -skx`; a full home scan of 3.8M files takes about 31 s.
- **Phase 3.5 (GPUI migration):** Tauri and React are gone. `backend.rs` holds plain functions on `AppState` (scan progress through a callback). `src/ui/` has the window (`mod.rs`, `root.rs`), one `AppModel` entity (`model.rs`), a live or mock data source (`source.rs`, `DISKLENS_MOCK=1` or `preselect`), pure logic with tests (`treemap.rs` squarify matching d3 exactly, `stats.rs`, `selection.rs`, `format.rs`, `theme.rs`), and the views (`screens.rs`, `stat_cards.rs`, `explorer.rs`, `recommendations.rs`). Geist fonts are embedded from `assets/fonts/`. The Rust toolchain is pinned to 1.99.1 (`rust-toolchain.toml`; GPUI needs 1.95 or later).
- Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` passes (188 tests).

## Next

1. Finish the Phase 3.5 review listed under "What is left" in `docs/PLAN.md`: interactions in the native window, and the mockup differences to decide. Only I can drive the native window; you can capture it with `screencapture -l <window id>` while the screen is unlocked (a locked screen captures blank).
2. Then Phase 4 (cleanup engine), starting in plan mode. The confirm dialog, progress and results go in `src/ui/cleanup.rs`; Copy script and Clean selected are disabled in the footer of `src/ui/recommendations.rs` until then.
