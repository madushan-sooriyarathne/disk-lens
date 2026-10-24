---
name: qcode
description: Execute the /qplan plan or a pasted EXECUTION HANDOFF without expanding scope, then validate and report results. Trigger with /qcode.
argument-hint: "[EXECUTION HANDOFF or notes]"
disable-model-invocation: true
allowed-tools:
  - Read
  - Edit
  - Write
  - Grep
  - Glob
  - Bash
  - Agent
---

# QCode

Execute an existing implementation plan precisely, validate the output, and report status. `/qcode`
executes; it does not plan or redesign. Before any work, load every rule and skill the plan names.

## Input & Plan Selection

Source priority:

1. A plan or `EXECUTION HANDOFF` block passed in `$ARGUMENTS`.
2. The most recent `/qplan` or `/qcheck` output in the session.
3. The newest `${TMPDIR:-/tmp}/qplan-*.md` or `qcheck-*.md` file. Show its Objective (or Task) and continue only if it matches
   the session's task; otherwise ask.

If no valid plan exists, halt and ask the user to run `/qplan` first — do not improvise one.

## Pre-Execution Checks

Before editing any file:

1. **Load required rules & skills.** Read every rule (e.g. `.claude/rules/*.md` / `.agents/rules/*.md`) and skill listed under the plan's `Required Rules & Skills` or instructions, plus the project instructions (`CLAUDE.md` / `AGENTS.md`). Obey them throughout.
2. **Verify the workspace baseline.** Branch, paths, and dependencies match the plan's assumptions.
3. **Read referenced files** before editing; confirm target APIs and interfaces match the plan.
4. **Reconcile discrepancies.** If the codebase materially differs from the plan, pause and reconcile before changing anything.
5. **Detect the project's commands.** Read `package.json` scripts (or the equivalent manifest — `Cargo.toml`, `go.mod`, `pyproject.toml`, `Makefile`) and config files. Prefer commands the instruction file names.

## Execution Constraints

- **Strict scope.** Implement the planned changes in sequence. Do not refactor unrelated code, reformat untouched files, or upgrade unrelated dependencies.
- **Preserve conventions.** Match the naming, error handling, and architecture of the surrounding code.
- **Minimal footprint.** Apply the smallest coherent diff that meets the acceptance criteria (the `ponytail` ladder).
- **Dependencies & generated files.** Add no dependency unless the plan says so. Never hand-edit generated files; edit their sources.

## Execution & Validation Loop

If the plan's `Browser Verification` asks for a baseline, capture it per
[Browser Check](#browser-check) before the first edit.

If the plan's `Execution Strategy` says `delegated`, follow [Delegated Execution](#delegated-execution).
Otherwise, for each planned step:

1. **Inspect** the target code.
2. **Implement** the planned change.
3. **Run targeted validation** (the relevant test, type-check, or lint). Do not defer all validation to the end.
4. **Reconcile failures.** Separate pre-existing breakage from regressions the change caused; fix regressions before moving on.

After the last step, run the project's full quality gate, skipping any step it lacks: type-check,
lint, test, build. If the instructions name a single gate (e.g. one `test`/`check` script), run that.

<!-- lang:commands=rust -->
**Rust:**

```bash
cargo build                    # build
cargo check --all-targets      # fast compile + type check
cargo clippy --all-targets --all-features -- -D warnings   # lint
cargo fmt --check              # format check (cargo fmt to fix)
cargo test                     # all tests: unit, integration, doc
cargo test <name>              # tests whose path contains <name>
```

Gate order: `cargo fmt --check` → `cargo clippy` → `cargo test`. Workspace: add `--workspace`; one crate: `-p <crate>`. Prefer aliases in `.cargo/config.toml`, `justfile`/`Makefile` targets, or CI jobs when the project defines them.
<!-- /lang -->

When the gate passes, run the plan's browser flows per [Browser Check](#browser-check).

## Browser Check

Run this in both inline and delegated mode when the plan's `Browser Verification` lists flows.
Re-check that the `agent-browser` CLI is on `PATH`; if it is missing, report the check as skipped
with that reason. Never report a skipped check as passed.

1. **Dispatch** one `haiku` subagent when the host takes a model per call; otherwise run the
   steps inline. Brief it with the URL, start command, flows, checks, and auth env var names.
2. **The subagent** loads the `agent-browser` skill and uses its commands. It starts the dev
   server in the background only if the URL does not respond, runs each flow, and stops only a
   server it started. It saves a screenshot only for a failed flow and returns the path, not the
   image. It never edits files.
3. **Receipt.** For each flow: pass or fail; for a failure, the failing step plus the raw output
   that shows it. Also console errors, failed requests, and any performance numbers measured.
4. **Handle failures.** Retry a failed flow once to rule out timing flakiness. Fix a real failure
   yourself (or return it to its unit's tier), then re-run only the failing flows. If the server
   does not start, report a blocker; do not claim done.

## Delegated Execution

Use this only when the plan's mode is `delegated` and the host has a subagent tool that takes a
model per call (Claude Code's `Agent` tool). Otherwise, for example on Antigravity, execute inline
as above. You are the orchestrator: you dispatch units, check them, and own the final result.

1. **Dispatch.** Start one subagent per unit, with the unit's `model` alias from the plan. Run
   independent units in parallel, in one message. Start a unit only after its dependencies finish.
2. **Brief each unit cold.** Give it only its own steps, its owned files, the plan's
   `Required Rules & Skills`, and its validation command. Tell it: edit only the owned files; add
   no dependency; do not commit; stop and report if the plan does not fit the code. Ask for a
   short receipt: files changed, validation command and result, deviations.
3. **Check each receipt.** Compare `git diff --stat` with the unit's owned files. An edit outside
   them is a deviation: reconcile it before you continue. Spot-read the risky hunks; do not
   re-read the whole diff.
4. **Run the gate.** If the plan's `Verification` names `haiku`, dispatch the full quality gate
   to a `haiku` subagent. It reports pass or fail, plus only the failing lines. It never edits
   files.
5. **Handle failures.** Fix small regressions yourself. Return a larger one to its unit's tier
   with the failing output. Re-run the gate after each fix.
6. **Run the browser check** per [Browser Check](#browser-check) once the gate passes.

## Deviations

Allowed only when an interface mismatch, missing dependency, or repository rule blocks the planned
approach. Then:

- Pick the smallest compatible adjustment that keeps the original objective.
- Document the rationale in the report.
- For architectural deviations, pause and confirm with the user.

## Final Report

Terse, per `caveman` and `i-have-adhd`. Never claim done on an unverified change.

On success:

```plaintext
Execution Complete

Implemented:
  <summary of completed work>

Files changed:
  <modified/added paths>

Validation:
  <command> → passed

Deviations:
  <none, or brief explanation>

Delegation:
  <inline, or each unit: model → files → validation result>

Browser:
  <none | skipped (reason) | each flow → passed/failed, perf vs baseline>

Notes:
  <operational follow-ups, or none>
```

On failure or blocker:

```plaintext
Execution Blocked

Implemented:
  <work completed so far>

Validation:
  <command> → failed

Failure:
  <root cause or failing output>

Remaining:
  <work still required to resolve>
```

End with one next action: `/qcheck` (review) or `/qgit` (commit) when green.
