---
name: qplan
description: Investigate the repository and turn a task into an implementation-ready plan plus a portable execution handoff, without modifying code. Trigger with /qplan followed by the task.
---

# QPlan

Plan the task in `$ARGUMENTS` so it fits the codebase instead of fighting it. Understand first,
then propose a grounded, actionable plan with the smallest change that works. Modify no project
files — the only write is the plan file below.

## Preconditions

Ensure `/qnew` has run. If not, first resolve the project instructions (`CLAUDE.md` / `AGENTS.md`)
and environment constraints before planning.

## Investigation Procedure

1. **Read the rules.** Load the project instructions (`CLAUDE.md` / `AGENTS.md`) plus any rules/guides they reference. The plan must obey them.
2. **Map the ground.** If `graphify-out/GRAPH_REPORT.md` exists, read it first for the god nodes and community structure before scanning raw files. Then grep for existing functionality, patterns, and naming conventions the task touches — trace the real flow end to end.
3. **Deconstruct the requirement.** Identify functional requirements, acceptance criteria, boundaries, and potential side effects.
4. **Inspect context.** Read entry points, relevant components, types, schemas, services, and existing tests. Trace the end-to-end data flow.
5. **Check patterns.** Match existing architectural paradigms and utilities. Prefer extending existing code over new files; no new abstractions when existing ones suffice.
6. **Determine validation.** Identify targeted verification (unit tests, integration tests, lint, type-check) in the project's existing test style:

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

7. **Determine required rules & skills.** Identify repository rules (e.g. path-scoped rules in `.claude/rules/` / `.agents/rules/`) and skills (e.g. `ponytail`) that `/qcode` must load before executing the plan.
8. **Decide the execution strategy.** Choose `inline` or `delegated` per [Execution Strategy](#execution-strategy).
9. **Plan browser verification.** If a browser check applies per [Browser Verification](#browser-verification), define its flows.
10. **Clarify ambiguities.** If an ambiguity materially changes implementation decisions, pause and ask targeted questions in a single grouped prompt. Document minor assumptions directly in the plan.

## Execution Strategy

Planning stays on the most capable model. Execution may fan out to cheaper subagents, but only
when that costs fewer tokens than the planning model doing the work itself.

**Model tiers.** Name tiers by alias (`opus`, `sonnet`, `haiku`), never by version: the host
resolves each alias to the newest model in that family.

- `sonnet`: implementation units (writing code and tests).
- `haiku`: mechanical units, such as running the quality gate (type-check, lint, test, build),
  browser checks, and reproducing a failure. These report results; they never fix code.

**Break-even rule.** Estimate token counts as chars/4 (`wc -c` on the files a unit reads, plus
the expected diff and validation output). Let `P` be the per-token price of each tier; as of
2026-10, `sonnet` costs about ½ of `opus` and `haiku` about 1/40.

- Inline cost: `W × P_opus`. `W` is all tokens the work reads and writes, including the context
  that each later turn carries forward.
- Delegated cost: `(H + M) × P_opus + Σ (C + W_unit) × P_unit`. `H` is the handoff prompts the
  orchestrator writes. `M` is monitoring: receipts, `git diff --stat`, and spot-reading risky
  hunks. `C` is each subagent's cold start (it re-reads its files and rules).
- Choose `delegated` only when delegated cost is clearly lower, about 70% of inline cost or
  less, because the estimates are rough. Otherwise choose `inline`.

**Unit constraints.** Each delegated unit owns a disjoint set of files, so no two subagents write
the same file. Each unit is specified completely enough to run cold, without this conversation.
A unit that fails these constraints merges into a neighbor or stays inline.

## Browser Verification

A browser check applies only when all three hold:

- The `agent-browser` skill is available (listed in the session, or present under
  `.claude/skills/agent-browser/` or `.agents/skills/agent-browser/`).
- `command -v agent-browser` succeeds. If the skill is present but the CLI is missing, record
  `browser check skipped: agent-browser CLI missing` in the plan. Never skip silently.
- The change has a browser-observable effect: UI components, pages, styles, routing, forms,
  client state, accessibility, or client performance (animation, LCP/INP/CLS, bundle size).
  A server-only or API-only change does not qualify.

Each flow names the page, the steps, and the expected result. For a performance change, also
ask for a baseline measurement before any edit, so the result has something to compare against.
Name credentials by environment variable only, never by value. Take command syntax from the
`agent-browser` skill; do not write it into the plan.

The browser check always runs on `haiku` when the host supports per-call models. No break-even
estimate is needed: page snapshots and console output make up most of its tokens, and `haiku`
costs about 1/40 of `opus`.

## Plan Structure

Build the plan with this schema. Enforce the `ponytail` ladder (YAGNI, smallest working diff, stdlib/platform first).

```markdown
# Implementation Plan

## Objective

<precise statement of the requested outcome>

## Required Rules & Skills

- **Rules**: `<rules to load before execution, e.g. .claude/rules/testing.md>`
- **Skills**: `<skills to load before execution, e.g. ponytail>`

## Current State vs. Proposed Changes

### 1. <Target component/feature>

- **Files**: `<path>`
- **Changes**: <specific modification details>
- **Rationale**: <why this change is needed>

## Edge Cases & Risks

- <identified edge case or operational risk, and its mitigation>

## Testing & Validation

- `<command>`: <behavior it validates>

## Acceptance Criteria

- [ ] <verifiable criterion>

## Execution Strategy

- **Mode**: inline | delegated
- **Estimate**: inline ≈ <tokens> on `opus`; delegated ≈ <orchestrator tokens> on `opus` + <subagent tokens> per tier
- **Units** (delegated only):
  - **U1**: model `sonnet`; files `<owned paths>`; steps <sequence numbers>; depends on <none | U#>; validate `<command>`
- **Verification**: model `haiku` | inline; `<gate commands>`; browser check per Browser Verification, or none

## Browser Verification

- **Status**: <flows below | none (reason) | skipped: agent-browser CLI missing>
- **URL**: `<base url>`; start with `<dev command>` unless already running
- **Baseline**: <before-edit measurement for performance changes, or none>
- **Flows**:
  - **F1** `<route>`: <steps> → <expected result>
- **Checks**: no console errors; no failed requests; <perf metric and threshold, if any>
- **Auth**: <env var names, or none>

## File Manifest

- **Modify**: `<paths>`
- **Create**: `<paths>`
- **Delete**: `<paths>`

## Implementation Sequence

1. <ordered, step-by-step execution list>
```

## Portable Handoff Contract

Always append a self-contained execution block, so `/qcode` or another agent can run the plan cold:

```plaintext
EXECUTION HANDOFF

Repository assumptions:
<key branch, environment, paths, tools>

Task:
<clear objective>

Required Rules & Skills:
- Rules: <rules to load before execution>
- Skills: <skills to load before execution>

Instructions:
<project-specific conventions and constraints>

Detailed plan:
<the implementation plan>

Validation:
<targeted validation commands>

Browser verification:
<the Browser Verification section, or none>

Execution strategy:
<the Execution Strategy section: mode, units, verification>

Execution rules:
<scope boundaries and specific restrictions>
```

## Presentation & Delivery

1. **Save the plan.** Write the detailed plan plus the EXECUTION HANDOFF to
   `${TMPDIR:-/tmp}/qplan-<task-slug>.md` — never inside the repository.
2. **Show a TL;DR only**, shaped per `caveman` and `i-have-adhd`: lead with the next action and a
   time estimate, then the approach and scope (files touched) in a few lines.
   Include one line for the execution strategy, e.g.
   `Execution: delegated (3 sonnet units + haiku gate)` or `Execution: inline`, and one for the
   browser check, e.g. `Browser: 2 flows (haiku)` or `Browser: none`.
3. **Offer two choices**, as a numbered list:
   1. **View detailed plan** — print the plan file.
   2. **Copy handoff prompt** — copy the plan file to the clipboard so another model can execute it
      (`pbcopy < file`, `wl-copy < file`, `xclip -selection clipboard < file`, or `clip.exe < file`,
      whichever exists). If none is available, render the handoff in a code block instead.
4. **Otherwise**, end by telling the user to run `/qcode` to execute the plan.
