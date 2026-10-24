# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

DiskLens: macOS disk space analyzer and safe cleaner. One Rust crate at the repo root: backend plus a native UI drawn with GPUI (Zed's GPU UI framework).

## Source of truth

Read these before any task. If they conflict or are unclear, ask the user; do not guess.
- `docs/PRD.md` - requirements, safety rules S-01..S-10, visual spec
- `docs/RULES.md` - classification and cleanup rules; section 7 is the starter `rules.toml`
- `docs/PLAN.md` - stack, repo layout, data contracts, backend functions, phases
- `docs/mockup.jpg` - target UI

Work one PLAN.md phase at a time: propose the task breakdown in plan mode, wait for approval, commit per task (conventional commits), finish with the phase acceptance checks and a short summary, then stop for review. Current status: Phases 0-3 committed; Phase 3.5 (Tauri and React replaced by GPUI) done on branch `gpui-migration`, awaiting review; do not start Phase 4 unprompted.

## Commands

```bash
cargo run                                   # run the app
DISKLENS_SCAN_ROOT=$PWD/.sandbox/home cargo run   # run against the fixture sandbox
DISKLENS_MOCK=1 cargo run                   # mock data (DISKLENS_MOCK=preselect also selects the mockup rows)
cargo fmt --check                           # cargo fmt to fix
cargo clippy --all-targets -- -D warnings
cargo test                                  # all tests; tests/sandbox_rows.rs needs .sandbox current
cargo test <name>                           # single test
scripts/make-fixture.sh                     # rebuild .sandbox/home (idempotent)
cargo run --release --bin scan-bench -- <path>   # read-only scan, compares size with du (1% goal)
```

Every phase ends with `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` passing.

## Architecture

Backend (`src/`):
- `lib.rs` declares the modules; `main.rs` calls `ui::run()`. `state.rs` holds `AppState` (config, current `Snapshot` and analysis, cancel flag of the single running scan).
- `backend.rs`: plain functions on `&Arc<AppState>` (`start_scan`, `cancel_scan`, `load_last_snapshot`, `get_view`, `get_recommendations`, `get_item_details`, `reveal_in_finder`). `start_scan` canonicalizes the scan root, runs `scanner::scan` on its own thread and reports `ScanEvent` (progress / finished / failed) through a callback. On success it saves the snapshot (zstd JSON, `scanner/snapshot.rs`), runs the analysis and swaps both into state as `Current`. `load_last_snapshot` does the same at startup.
- `scanner/`: `walk.rs` (parallel lstat walk, no symlink following, same-device only), `hardlinks.rs` (each inode counted once), `tree.rs` (arena `Vec<DirNode>` with bottom-up sizes; also the contract types), `volume.rs` (capacity).
- `rules/`: `rules/rules.toml` (repo root) embedded and validated at startup (`model`, `load`, `validate`), `matcher` (one DFS per snapshot, RULES.md 4.1), `category` (section 6, markers checked lazily on disk), `guards` (match-time, blocking, safety), `tools` (login-shell PATH; sandbox mode resolves only `git`).
- `platform/`: `System` (processes, running apps) and `Runner` (argv commands with timeout) traits with real and fake implementations; tests describe the machine with the fakes.
- `providers/`: ollama, git worktrees, simctl and docker item enumeration. `cleanup/protected.rs`: hardcoded protect list (equality test with `[[protect]]`).
- `recommend/build.rs`: `analyze` (matches, providers, min size) runs once per snapshot; rows are rebuilt on every `get_recommendations` because blocking guards depend on what runs now. `get_item_details` adds description, regenerates and largest subfolders.
- `view.rs`: `get_view` builds the treemap model on demand from the snapshot: the requested folder plus two levels, small siblings merged into "Other", with category, tier and rule filled from the analysis. Paths from the UI are resolved against the snapshot (`validated_path`); anything outside it is refused.

UI (`src/ui/`, GPUI):
- `mod.rs` opens the window and loads the fonts; `root.rs` renders `AppModel` by phase (loading / idle / scanning / ready / error).
- `model.rs`: `AppModel`, the single GPUI entity (phase, snapshot meta, explorer path and mode, recommendations, selected item IDs, drawer row). Backend calls run off the UI thread; recommendations refresh on window activation.
- `source.rs`: `Source::Live` (calls `backend.rs`) or `Source::Mock` (`assets/mock/snapshot.mock.json`, chosen by `DISKLENS_MOCK`).
- Pure logic with unit tests beside it: `treemap.rs` (squarified layout ported from d3), `stats.rs`, `selection.rs`, `format.rs`, `theme.rs` (tokens; category colors from `rules.toml`).
- Views stay presentational: `screens.rs`, `stat_cards.rs`, `explorer.rs`, `recommendations.rs`.

## Safety rules (non-negotiable)

- Never delete, trash or run cleanup against the real home folder. Develop against the fixture sandbox: `DISKLENS_SCAN_ROOT=$PWD/.sandbox/home cargo run` (absolute path).
- Debug builds default to `DISKLENS_DRY_RUN=1` (see `src/config.rs`).
- Do not run `rm`, `git clean` or any cleanup command outside this repository.
- Real cleanup execution is enabled only in Phase 4, on a single safe item (the npm cache), and only when the user explicitly says so.
- `scripts/make-fixture.sh` may only delete `<repo>/.sandbox`; keep its path guard.
- No TODOs in safety-related code (`cleanup/`, `rules/guards.rs`, `rules/matcher.rs`).

## Conventions

- Latest stable dependency versions; confirm configuration against official docs and crate docs rather than memory. GPUI is pinned to an exact snapshot (`gpui-pre` / `gpui-pre-platform` `=0.3.9`, imported as `gpui` / `gpui_platform`); read its source and `examples/` in `~/.cargo/registry/src/*/gpui-pre-0.3.9/` for API questions, and bump it only in its own commit. The Rust toolchain is pinned in `rust-toolchain.toml` (GPUI needs 1.95 or later).
- `rules/rules.toml` is the TOML block of `docs/RULES.md` section 7 verbatim. Change both, plus tests, in the same commit. `tests/rules_doc_sync.rs` enforces equality.
- Cleanup actions send item IDs, never paths (S-01). Explorer actions (`get_view`, `reveal_in_finder`) take paths, which the backend validates against the snapshot.
- `assets/mock/snapshot.mock.json` mirrors `docs/mockup.jpg` numbers (test in `src/ui/source.rs`); its `preselected` list is mock-only, real default selection is empty.
- Fixture sandbox (`.sandbox/`, gitignored) contains every rule target, near misses, protected paths, a symlink out, hard links, git repos and worktrees (one dirty), and idle/active projects.

## Environment variables

| Var                  | Effect                                                                                                                                                                             |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DISKLENS_SCAN_ROOT` | Scan root override (dev only, absolute path to `.sandbox/home`)                                                                                                                    |
| `DISKLENS_DRY_RUN`   | `1`/`true` log actions without performing; `0`/`false` perform. Unset = on in debug builds, off in release                                                                         |
| `DISKLENS_DATA_DIR`  | Snapshot/settings dir. Default: `<scan root>/../data` when `DISKLENS_SCAN_ROOT` is set (so sandboxed runs write to `.sandbox/data`), else `~/Library/Application Support/DiskLens` |
| `DISKLENS_MOCK`      | `1` serves the mock snapshot instead of scanning; `preselect` also selects the mockup's rows                                                                                       |

## Workflow

- Run `cargo check --all-targets` after a series of code changes; the full gate is in Commands.
- Fix the root cause, not the symptom.
- When unsure about approach, use plan mode (`Shift+Tab`) before coding.

## Don'ts

- Don't hand-edit generated files; regenerate them.

# Communication style

Caveman mode (`caveman` skill): terse replies, technical substance intact. Switch with `/caveman lite|full|ultra`; say "stop caveman" to revert.

# Build discipline

Ponytail mode (`ponytail` skill): smallest working change, reuse before writing. Switch with `/ponytail lite|full|ultra`; say "stop ponytail" to revert.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
