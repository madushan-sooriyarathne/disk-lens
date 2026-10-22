# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

DiskLens: macOS disk space analyzer and safe cleaner. Tauri v2 (Rust backend, `src-tauri/`) + React 19 / TypeScript / Tailwind v4 (frontend, `src/`).

## Source of truth

Read these before any task. If they conflict or are unclear, ask the user; do not guess.
- `docs/PRD.md` - requirements, safety rules S-01..S-10, visual spec
- `docs/RULES.md` - classification and cleanup rules; section 7 is the starter `rules.toml`
- `docs/PLAN.md` - stack, repo layout, data contracts, tauri commands, phases
- `docs/mockup.jpg` - target UI

Work one PLAN.md phase at a time: propose the task breakdown in plan mode, wait for approval, commit per task (conventional commits), finish with the phase acceptance checks and a short summary, then stop for review. Current status: Phase 0 complete once reviewed; do not start Phase 1 unprompted.

## Commands

```bash
pnpm install
pnpm tauri dev                 # run app (Vite on :1420 + Rust)
pnpm build                     # tsc + vite build
pnpm lint                      # biome check .   (pnpm format = --write)
pnpm test                      # vitest run
pnpm vitest run src/lib/format.test.ts      # single TS test file
pnpm test:rust                 # cargo test in src-tauri
cargo test --manifest-path src-tauri/Cargo.toml <name>   # single Rust test
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
scripts/make-fixture.sh        # rebuild .sandbox/home (idempotent)
```

Every phase ends with `pnpm lint && pnpm test && cargo test` plus clippy passing.

## Safety rules (non-negotiable)

- Never delete, trash or run cleanup against the real home folder. Develop against the fixture sandbox: `DISKLENS_SCAN_ROOT=./.sandbox/home`.
- Debug builds default to `DISKLENS_DRY_RUN=1` (see `src-tauri/src/config.rs`).
- Do not run `rm`, `git clean` or any cleanup command outside this repository.
- Real cleanup execution is enabled only in Phase 4, on a single safe item (the npm cache), and only when the user explicitly says so.
- `scripts/make-fixture.sh` may only delete `<repo>/.sandbox`; keep its path guard.
- No TODOs in safety-related code (`cleanup/`, `rules/guards.rs`, `rules/matcher.rs`).

## Conventions

- Latest stable dependency versions; confirm configuration against official docs (Tauri v2, Tailwind v4, Biome v2, crate docs) rather than memory.
- `src-tauri/rules/rules.toml` is the TOML block of `docs/RULES.md` section 7 verbatim. Change both, plus tests, in the same commit. `src-tauri/tests/rules_doc_sync.rs` enforces equality.
- TS types for Rust contracts are generated with `ts-rs` into `src/lib/bindings/` (excluded from Biome); never hand-edit them.
- The frontend sends item IDs, never paths (S-01).
- Biome formats with tabs and double quotes; Tailwind directives are enabled for CSS parsing.
- `src/mocks/snapshot.mock.json` mirrors `docs/mockup.jpg` numbers (test in `snapshot.mock.test.ts`); its `preselected` list is mock-only, real default selection is empty.
- Fixture sandbox (`.sandbox/`, gitignored) contains every rule target, near misses, protected paths, a symlink out, hard links, git repos and worktrees (one dirty), and idle/active projects.

## Environment variables

| Var                  | Effect                                                                                                                                                                             |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DISKLENS_SCAN_ROOT` | Scan root override (dev only, point at `./.sandbox/home`)                                                                                                                          |
| `DISKLENS_DRY_RUN`   | `1`/`true` log actions without performing; `0`/`false` perform. Unset = on in debug builds, off in release                                                                         |
| `DISKLENS_DATA_DIR`  | Snapshot/settings dir. Default: `<scan root>/../data` when `DISKLENS_SCAN_ROOT` is set (so sandboxed runs write to `.sandbox/data`), else `~/Library/Application Support/DiskLens` |
