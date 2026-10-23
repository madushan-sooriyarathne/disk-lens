# DiskLens handoff prompt

Paste everything below the line into a new Claude Code session opened in `/Users/madushan/Developer/disk-lense`.

---

You are continuing work on DiskLens, a macOS disk space analyzer and safe cleaner (Tauri v2 Rust backend in `src-tauri/`, React 19 + TypeScript + Tailwind v4 frontend in `src/`).

## Start here

1. Read `CLAUDE.md`, then `docs/PRD.md`, `docs/RULES.md`, `docs/PLAN.md` in full. They are the source of truth. If they conflict or something is unclear, ask me instead of guessing. `docs/mockup.jpg` is the target UI.
2. Run `git status` and `git log --oneline` to confirm state. Phases 0 to 3 are committed locally (nothing pushed). The tree should be clean.
3. Read "Status and roadmap" at the top of `docs/PLAN.md` for what is left.

## How we work (unchanged)

- Follow `docs/PLAN.md` one phase at a time. Before writing code for a phase, enter plan mode, propose the task breakdown, and wait for my approval.
- Commit after each task with conventional commit messages (end them with the Co-Authored-By line your environment specifies).
- At the end of each phase run its acceptance checks, give me a short summary of what was built, what passed, and what you could not verify, then stop for my review.
- Use the latest stable dependency versions and check official docs (Tauri v2, Tailwind v4, Biome v2, crate docs) instead of relying on memory.
- Replies are terse (caveman style) but commits, code and docs are normal prose.

## Safety rules (non-negotiable)

- Never delete, trash or run cleanup against my real home folder. Develop against the fixture sandbox: `DISKLENS_SCAN_ROOT=$PWD/.sandbox/home pnpm tauri dev` (absolute path, because `tauri dev` runs cargo from `src-tauri/`). Rebuild the sandbox with `scripts/make-fixture.sh`.
- Debug builds default to `DISKLENS_DRY_RUN=1` (`src-tauri/src/config.rs`).
- Do not run `rm`, `git clean` or any cleanup command outside this repository.
- Real cleanup execution is enabled only in Phase 4, on one safe item (the npm cache), and only when I explicitly say so.
- Read-only scans of real folders with `scan-bench` are fine (they write nothing).
- `src-tauri/rules/rules.toml` must stay verbatim equal to the TOML block in `docs/RULES.md` section 7. Change both, plus tests, in the same commit (`src-tauri/tests/rules_doc_sync.rs` enforces it).

## What is done

- **Phase 0 (scaffold):** Tauri v2 app, Biome v2, Vitest, Tailwind v4, `CLAUDE.md`, `scripts/make-fixture.sh` (idempotent sandbox at `.sandbox/home`, only deletes `<repo>/.sandbox`), `src/mocks/snapshot.mock.json` (mirrors the mockup), `config.rs` with dry-run default.
- **Phase 1 (scanner):** `src-tauri/src/scanner/` with `walk.rs` (jwalk, lstat only, no symlink following, same-device only, dataless files count 0), `hardlinks.rs` (first sighting in sorted order wins, no stored paths), `tree.rs` (arena `DirNode`, top 20 files per directory), `volume.rs` (NSURL capacity, statfs fallback), `snapshot.rs` (zstd JSON, atomic write), commands `start_scan`, `cancel_scan`, `load_last_snapshot`, and the `scan-bench` binary. `DISKLENS_DATA_DIR` redirects data to `.sandbox/data` when a scan root override is set. Verified: within 1% of `du -skx` on every folder tried, a full home scan of 3.8M files in about 31 s, peak memory about 444 MB.
- **Phase 2 (Explorer UI):** `view.rs` (`get_view`, `validated_path`, `reveal_in_finder`), recommendation/view model types with generated TS bindings in `src/lib/bindings/` (ts-rs, never hand-edit), app shell, stat cards (`lib/stats.ts`), squarified treemap (`lib/treemap.ts`, d3-hierarchy), breadcrumb, legend, tooltip, context menu, scan progress / idle / error states, mock mode (`pnpm dev`, then `http://localhost:1420/?mock&preselect`).
- **Phase 3 (rules engine and recommendations):** `src-tauri/src/rules/` (model, loader, validation, matcher, category, guards, tools), `platform/` (`System` and `Runner` traits with fakes), providers (ollama manifests, git worktrees, simctl, docker; none of the global-tool providers run in sandbox mode), `cleanup/protected.rs` (list plus equality test), `recommend/build.rs`, commands `get_recommendations` and `get_item_details`, Recommendations panel, detail drawer, selection store, tree hatching. Copy script and Clean selected render but stay disabled until Phase 4.
- Tests: 31 Vitest, 142 Rust. `pnpm lint && pnpm test && cargo test --manifest-path src-tauri/Cargo.toml` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` all pass.

I approved Phase 2 after running it in the native window. The built-in browser pane can drive mock mode, but nobody can drive the native Tauri window except me, so ask me to eyeball real-window behaviour.

## Next: Phase 4 (cleanup engine). Not started, no plan yet.

Phase 3 is committed and every automated check passes. Before Phase 4, the mock-mode panel still needs a side-by-side check against the mockup in the browser pane, and I check real rows in the native window.

The remaining work for Phases 4 to 6 is listed in `docs/PLAN.md` under "Status and roadmap". Start Phase 4 by entering plan mode, proposing the task breakdown, and waiting for my approval.
