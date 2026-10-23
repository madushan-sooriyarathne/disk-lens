# DiskLens handoff prompt

Paste everything below the line into a new Claude Code session opened in `/Users/madushan/Developer/disk-lense`.

---

You are continuing work on DiskLens, a macOS disk space analyzer and safe cleaner (Tauri v2 Rust backend in `src-tauri/`, React 19 + TypeScript + Tailwind v4 frontend in `src/`).

## Start here

1. Read `CLAUDE.md`, then `docs/PRD.md`, `docs/RULES.md`, `docs/PLAN.md` in full. They are the source of truth. If they conflict or something is unclear, ask me instead of guessing. `docs/mockup.jpg` is the target UI.
2. Run `git status` and `git log --oneline` to confirm state. Phases 0, 1 and 2 are committed locally (nothing pushed). The tree should be clean.
3. Read the approved Phase 3 plan at `/Users/madushan/.claude/plans/buzzing-sauteeing-toucan.md` (summary below in case it is not available).

## How we work (unchanged)

- Follow `docs/PLAN.md` one phase at a time. Before writing code for a phase, enter plan mode, propose the task breakdown, and wait for my approval. Phase 3's plan is already approved, so you can start it directly.
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
- **Phase 2 (Explorer UI):** `view.rs` (`get_view`, `validated_path`, `reveal_in_finder`), recommendation/view model types with generated TS bindings in `src/lib/bindings/` (ts-rs, never hand-edit), app shell, stat cards (`lib/stats.ts`), squarified treemap (`lib/treemap.ts`, d3-hierarchy), breadcrumb, legend, tooltip, context menu, scan progress / idle / error states, mock mode (`pnpm dev`, then `http://localhost:1420/?mock&preselect`). The right card is an empty "Recommendations" placeholder.
- Tests: 22 Vitest, 52 Rust. `pnpm lint && pnpm test && cargo test --manifest-path src-tauri/Cargo.toml` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` all pass.

I approved Phase 2 after running it in the native window. The built-in browser pane can drive mock mode, but nobody can drive the native Tauri window except me, so ask me to eyeball real-window behaviour.

## Next: Phase 3 (rules engine and recommendations). Plan approved, no code written yet.

Goal: parse and validate `rules.toml`, resolve categories, match rules over the snapshot, evaluate guards, run providers, build recommendation rows, and add the Recommendations panel, detail drawer and tree hatching. Nothing is deleted or executed (that is Phase 4); every new tool or process call is read-only.

Decisions I already confirmed:

- **Media bytes:** the scanner records `media_bytes` per directory. `DirNode` gains `media_bytes` (`#[serde(default)]`, old snapshots read as 0). `scan()` takes the `media_extensions` set from rules.toml. TS bindings regenerate.
- **Sandbox safety:** when `DISKLENS_SCAN_ROOT` is set, only `git` resolves as a tool. `pnpm`, `brew`, `ollama`, `docker`, `xcrun` do not, and the docker, ollama and simctl providers return nothing. Rules that need them use `fallback_method` or are hidden. This guarantees a sandbox run can never aim a global command (`pnpm store prune`, `docker system prune`, `ollama rm`) at the real machine.
- **Ollama:** enumerate and size models from `~/.ollama/models/manifests` only (no `ollama list`). The `requires_running` guard still blocks removal when the server is down. Update the `docs/RULES.md` section 4.3 provider row in the same commit.
- Copy script and Clean selected render in the footer but stay disabled until Phase 4. Dark mode is Phase 5.

Design (details in the plan file):

- `src-tauri/src/rules/`: `model.rs` (serde with `deny_unknown_fields`), `load.rs` (`Rules::embedded()` via `include_str!`), `validate.rs` (every failure in RULES.md section 3), `matcher.rs` (RULES.md 4.1: `~` expands to the glob-escaped scan root, one combined `GlobSet`, `literal_separator(true)`, single DFS with subtree consumption, highest priority then longest literal prefix, stop at protected / symlink / mount / inaccessible nodes), `category.rs` (RULES.md section 6; markers checked lazily via the filesystem and memoised; media fallback uses `media_bytes / size >= 0.6`), `guards.rs`, `tools.rs` (login-shell PATH with 3 s timeout plus known dirs).
- `System` and `Runner` traits abstract processes/apps (real: `sysinfo` and `NSWorkspace` via `objc2-app-kit`) and command execution (real: argv, absolute binary, minimal env, 10 min cap), with fakes for tests.
- `cleanup/protected.rs` is created now with the hardcoded protect list and a test that it equals the `[[protect]]` paths in rules.toml; Phase 4 extends it.
- `providers/`: `ollama`, `git_worktrees`, `simctl_unavailable`, `docker`.
- `recommend/build.rs`: rows, `split_rows_by = project`, `min_size_mb`, method incl. fallback, blocked status and reasons ("Quit Xcode to clean", "Start Docker Desktop to clean", "Has uncommitted changes"), item ids `"{rule_id}:{node_id}"`, rows sorted by size within tier. `Analysis` lives in `AppState` beside the snapshot. Blocking guards are re-evaluated on every `get_recommendations`.
- New commands: `get_recommendations`, `get_item_details {row_id}`. `get_view` fills `category`, `tier`, `rule_id` from the analysis.
- Frontend: `components/Recommendations/{Panel,Row,Footer,DetailDrawer}.tsx` per PRD 7.4 and 7.5, selection store upgrade (per-item toggle, tri-state row checkbox, `selectSafe`, blocked items never selectable, selection cleared on rescan), real `getRecommendations`/`getItemDetails` in `src/lib/api.ts` and the mock.

Tasks (one commit each): 1 rules model/loader/validation; 2 scanner `media_bytes`; 3 protected paths and category resolver; 4 tools, system probe, guards; 5 rule matcher plus `tests/rules.rs`; 6 providers; 7 recommendation builder and commands; 8 sandbox fixture rows (fixture writes rule targets of at least 1 MB, 50 MB+ for `app-caches`, ollama manifests, worktree `.git` files) plus `tests/sandbox_rows.rs`; 9 panel and selection store; 10 detail drawer, focus refresh, "Show recommendation"; 11 RULES.md notes.

Acceptance: lint, Vitest, cargo test and clippy pass; table-driven `tests/rules.rs` with a match and a non-match for every rule id plus overlap cases (Yarn vs `app-caches`); validation tests reject malformed TOML; protect-list equality test; the sandbox yields the expected rows and totals; mock-mode panel compared with the mockup in the browser pane; ask me to check real rows in the native window.

Known risks to test early: globset semantics for `**` and `/**` (in particular whether `**/node_modules/**` matches the `node_modules` folder itself; pin with tests and adapt the matcher, not the doc), matcher speed on about 500k directories, and container folders like `Desktop` not matching the mockup's colors (expected, flag it).

## Where the previous session stopped

I had just approved the Phase 3 plan and the first action was `cargo add toml globset sysinfo wait-timeout` plus `objc2-app-kit` (features `NSWorkspace`, `NSRunningApplication`). I interrupted that tool call, so no Phase 3 dependencies or code have been added yet. Start at Phase 3 task 1. Check whether `globset` exposes an `escape` helper before relying on it.

## Later phases (from `docs/PLAN.md`)

- Phase 4: cleanup engine (protected checks, plan, revalidate S-03, execute, script export, audit log, ConfirmDialog/Progress/Results, adversarial tests). One supervised real run on the npm cache only, when I say so explicitly.
- Phase 5: permissions (Full Disk Access onboarding), persistence, dark mode, accessibility, performance pass (2M-file synthetic tree, memory target under 400 MB).
- Phase 6: packaging, signing, notarization, universal build, updater.
