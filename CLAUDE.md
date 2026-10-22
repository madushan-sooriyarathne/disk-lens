# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

DiskLens: macOS disk space analyzer and safe cleaner. Tauri v2 (Rust backend, `src-tauri/`) + React 19 / TypeScript / Tailwind v4 (frontend, `src/`).

## Source of truth

Read these before any task. If they conflict or are unclear, ask the user; do not guess.
- `docs/PRD.md` - requirements, safety rules S-01..S-10, visual spec
- `docs/RULES.md` - classification and cleanup rules; section 7 is the starter `rules.toml`
- `docs/PLAN.md` - stack, repo layout, data contracts, tauri commands, phases
- `docs/mockup.jpg` - target UI

Work one PLAN.md phase at a time: propose the task breakdown in plan mode, wait for approval, commit per task (conventional commits), finish with the phase acceptance checks and a short summary, then stop for review. Current status: Phases 0-3 (scaffold, scanner, explorer UI, rules engine and recommendations) committed; do not start Phase 4 unprompted.

## Commands

```bash
pnpm install
pnpm tauri dev                 # run app (Vite on :1420 + Rust)
pnpm dev                       # frontend only in a browser; http://localhost:1420/?mock[&preselect] uses mock data
pnpm build                     # tsc + vite build
pnpm lint                      # biome check .   (pnpm format = --write)
pnpm test                      # vitest run
pnpm vitest run src/lib/format.test.ts      # single TS test file
pnpm test:rust                 # cargo test in src-tauri
cargo test --manifest-path src-tauri/Cargo.toml <name>   # single Rust test
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
scripts/make-fixture.sh        # rebuild .sandbox/home (idempotent); tests/sandbox_rows.rs needs it current
cargo run --release --manifest-path src-tauri/Cargo.toml --bin scan-bench -- <path>   # read-only scan, compares size with du (1% goal)
```

Every phase ends with `pnpm lint && pnpm test && cargo test` plus clippy passing.

## Architecture

Backend (`src-tauri/src/`):
- `lib.rs` builds one `Arc<AppState>` (`state.rs`: config, current `Snapshot`, cancel flag of the single running scan) and registers the commands in `commands.rs`.
- `start_scan` canonicalizes the scan root, runs `scanner::scan` on `spawn_blocking`, and streams `ScanEvent` (progress / finished / failed) over a Tauri `Channel`. On success it saves the snapshot (zstd JSON, `scanner/snapshot.rs`), runs the analysis and swaps both into state as `Current`. `load_last_snapshot` does the same at startup.
- `scanner/`: `walk.rs` (parallel lstat walk, no symlink following, same-device only), `hardlinks.rs` (each inode counted once), `tree.rs` (arena `Vec<DirNode>` with bottom-up sizes; also the contract types), `volume.rs` (capacity).
- `rules/`: `rules.toml` embedded and validated at startup (`model`, `load`, `validate`), `matcher` (one DFS per snapshot, RULES.md 4.1), `category` (section 6, markers checked lazily on disk), `guards` (match-time, blocking, safety), `tools` (login-shell PATH; sandbox mode resolves only `git`).
- `platform/`: `System` (processes, running apps) and `Runner` (argv commands with timeout) traits with real and fake implementations; tests describe the machine with the fakes.
- `providers/`: ollama, git worktrees, simctl and docker item enumeration. `cleanup/protected.rs`: hardcoded protect list (equality test with `[[protect]]`).
- `recommend/build.rs`: `analyze` (matches, providers, min size) runs once per snapshot; rows are rebuilt on every `get_recommendations` because blocking guards depend on what runs now. `get_item_details` adds description, regenerates and largest subfolders.
- `view.rs`: `get_view` builds the treemap model on demand from the snapshot: the requested folder plus two levels, small siblings merged into "Other", with category, tier and rule filled from the analysis. Paths from the frontend are resolved against the snapshot (`validated_path`); anything outside it is refused.

Frontend (`src/`):
- `lib/api.ts` defines the `Api` interface with a Tauri implementation and `lib/mockApi.ts` (backed by `mocks/snapshot.mock.json`). The mock is used when not running inside Tauri or with `?mock`.
- Zustand stores: `store/scan.ts` drives the app phase (loading / idle / scanning / ready / error) that `App.tsx` switches on; `store/view.ts` holds the explorer path and mode; `store/selection.ts` holds recommendations, selected item IDs (ready items only; pure logic in `lib/selection.ts`) and the drawer row; it refreshes on window focus.
- Pure logic lives in `lib/` (`treemap.ts` layout with d3-hierarchy, `stats.ts`, `format.ts`) with Vitest tests beside it; components stay presentational.

## Safety rules (non-negotiable)

- Never delete, trash or run cleanup against the real home folder. Develop against the fixture sandbox: `DISKLENS_SCAN_ROOT=$PWD/.sandbox/home pnpm tauri dev` (use an absolute path: `tauri dev` runs cargo from `src-tauri/`).
- Debug builds default to `DISKLENS_DRY_RUN=1` (see `src-tauri/src/config.rs`).
- Do not run `rm`, `git clean` or any cleanup command outside this repository.
- Real cleanup execution is enabled only in Phase 4, on a single safe item (the npm cache), and only when the user explicitly says so.
- `scripts/make-fixture.sh` may only delete `<repo>/.sandbox`; keep its path guard.
- No TODOs in safety-related code (`cleanup/`, `rules/guards.rs`, `rules/matcher.rs`).

## Conventions

- Latest stable dependency versions; confirm configuration against official docs (Tauri v2, Tailwind v4, Biome v2, crate docs) rather than memory.
- `src-tauri/rules/rules.toml` is the TOML block of `docs/RULES.md` section 7 verbatim. Change both, plus tests, in the same commit. `src-tauri/tests/rules_doc_sync.rs` enforces equality.
- TS types for Rust contracts are generated with `ts-rs` into `src/lib/bindings/` (excluded from Biome); never hand-edit them. Types marked `#[ts(export)]` are regenerated by `cargo test` (export dir set in `src-tauri/.cargo/config.toml`); commit the regenerated files with the Rust change.
- Cleanup actions send item IDs, never paths (S-01). Explorer commands (`get_view`, `reveal_in_finder`) take paths, which the backend validates against the snapshot.
- Biome formats with tabs and double quotes; Tailwind directives are enabled for CSS parsing.
- `src/mocks/snapshot.mock.json` mirrors `docs/mockup.jpg` numbers (test in `snapshot.mock.test.ts`); its `preselected` list is mock-only, real default selection is empty.
- Fixture sandbox (`.sandbox/`, gitignored) contains every rule target, near misses, protected paths, a symlink out, hard links, git repos and worktrees (one dirty), and idle/active projects.

## Environment variables

| Var                  | Effect                                                                                                                                                                             |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DISKLENS_SCAN_ROOT` | Scan root override (dev only, absolute path to `.sandbox/home`)                                                                                                                    |
| `DISKLENS_DRY_RUN`   | `1`/`true` log actions without performing; `0`/`false` perform. Unset = on in debug builds, off in release                                                                         |
| `DISKLENS_DATA_DIR`  | Snapshot/settings dir. Default: `<scan root>/../data` when `DISKLENS_SCAN_ROOT` is set (so sandboxed runs write to `.sandbox/data`), else `~/Library/Application Support/DiskLens` |
