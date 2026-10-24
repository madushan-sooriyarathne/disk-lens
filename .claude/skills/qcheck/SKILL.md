---
name: qcheck
description: Senior-engineer review of a scope (diff, branch, or path) that hunts silent bugs, missed edge cases, performance problems, and structural debt, then offers a fix plan for /qcode. Trigger with /qcheck.
argument-hint: "[diff | branch | path]"
disable-model-invocation: true
allowed-tools:
  - Read
  - Write
  - Grep
  - Glob
  - Bash(git status)
  - Bash(git log *)
  - Bash(git diff *)
  - Bash(wc *)
  - Bash(agent-browser *)
  - Bash(pbcopy *)
  - Bash(wl-copy *)
  - Bash(xclip *)
  - Bash(clip.exe *)
  - Agent
---

# QCheck

Review the target scope as a skeptical senior engineer / software architect, diagnose the real
risks, and turn them into an actionable fix plan. Look for problems — do not rubber-stamp. Modify
no project files — the only write is the fix plan file below.

## Input & Scope Detection

Interpret `$ARGUMENTS`:

- **Diff / working tree:** uncommitted changes (`git diff`, `git diff --cached`).
- **Branch:** divergence from the base branch (e.g. `git diff main...HEAD`).
- **Feature / component / directory / file:** the named paths or subsystem.

With no argument, review the current staged and unstaged changes.

## Preconditions

Before analyzing the code:

1. **Load instructions & rules.** Resolve the project instructions (`CLAUDE.md` / `AGENTS.md`) and read any `.claude/rules/*.md` / `.agents/rules/*.md` and files under `guides/` or `docs/` they point to. Judge the code against these, not invented standards.
2. **Load skills.** Load `ponytail` when installed (optional — skip if missing), plus any installed skill relevant to the reviewed stack or subsystem.
3. **Map the ground.** If `graphify-out/GRAPH_REPORT.md` exists, read it first for the god nodes and community structure before scanning raw files. Then grep for existing functionality, patterns, and naming conventions the scope touches — trace the real flow end to end.
4. **Determine execution rules & skills.** Identify the rules (e.g. `.claude/rules/testing.md`) and skills (e.g. `ponytail`) that `/qcode` must load before executing the fix plan.
5. **Decide the review strategy.** Review inline, or delegate per [Delegated Review](#delegated-review).
6. **Decide the browser check.** Run one per [Browser Check](#browser-check) when it applies.

## Delegated Review

Delegate only when the host has a subagent tool that takes a model per call (Claude Code's
`Agent` tool) and delegation costs fewer tokens than reviewing inline. Otherwise, for example on
Antigravity, review inline.

**Break-even rule.** Estimate the scope's size as chars/4 (`wc -c` on the changed files plus the
diff). Each reviewer reads the whole scope, so every added reviewer adds that cost again.

- Inline cost: `scope × P_opus`.
- Delegated cost: `(briefs + findings + spot-checks) × P_opus + Σ reviewers (scope + cold start) × P_sonnet`.
  As of 2026-10, `sonnet` costs about ½ of `opus` per token.
- Delegate only when delegated cost is about 70% of inline cost or less. In practice that means
  one or two reviewers on a large scope. Never fan out every reviewer by default.

**Pick reviewers by what the scope touches.** Use the project's review agents
(`.claude/agents/`) when installed, or else a general subagent with that reviewer's focus:

- `code-reviewer`: any code change (the default single reviewer).
- `security-reviewer`: auth, input handling, secrets, injection surfaces.
- `silent-failure-hunter`: error handling, fallbacks, retries, async flows.
- `performance-reviewer`: hot paths, queries, loops over large data.

Run the chosen reviewers in parallel with the `sonnet` model alias, read-only. Give each one the
scope and the loaded rules. Verify each finding against the cited `path:line` before you report
it, drop duplicates and false positives, then continue with [Output](#output).

## Browser Check

A browser check applies only when all three hold:

- The `agent-browser` skill is available (listed in the session, or present under
  `.claude/skills/agent-browser/` or `.agents/skills/agent-browser/`).
- `command -v agent-browser` succeeds. If the skill is present but the CLI is missing, say
  `browser check skipped: agent-browser CLI missing` in the output. Never skip silently.
- The scope has a browser-observable effect: UI components, pages, styles, routing, forms,
  client state, accessibility, or client performance (animation, LCP/INP/CLS, bundle size).

Derive the flows from the routes and components in scope: each flow names the page, the steps,
and the expected result. Dispatch them to one `haiku` subagent when the host takes a model per
call; otherwise run them inline. No break-even estimate is needed: page snapshots and console
output make up most of the tokens, and `haiku` costs about 1/40 of `opus`.

The subagent loads the `agent-browser` skill and uses its commands. It starts the dev server in
the background only if the URL does not respond, and stops only a server it started. It never
edits files. For each flow it returns pass or fail, the failing step with the raw output that
shows it, console errors, failed requests, and any performance numbers. It saves a screenshot
only for a failed flow and returns the path, not the image.

Retry a failed flow once to rule out timing flakiness. Trace each remaining failure to the code
path that causes it before reporting it as a `[Browser]` finding.

## Review Focus Areas

### Correctness

- **Off-by-one:** indexing one past the end, `i <= n` vs `i < n`, inclusive vs exclusive ranges, fence-post errors (n items need n-1 separators).
- **Missing values:** using a possibly-absent value (null/undefined/None/nil) without a check.
- **Logic:** inverted conditions, short-circuits skipping side effects, mutation of shared references, missing `break`/fallthrough in a switch (unless intentional and commented).
- **Race conditions:** shared mutable state across concurrent tasks, read-then-write without atomicity, handlers or tasks registered without cleanup.
- **Lifecycle:** state drift, memory leaks, improper setup/teardown.

<!-- lang:review=rust -->
**Rust:**

- `unwrap()`/`expect()`/slice indexing on input-derived data in non-test code.
- Lossy `as` casts (`u64 as u32`, `i64 as usize`): use `try_from`. Unchecked arithmetic on untrusted numbers.
- `.clone()` added to silence the borrow checker; `&String`/`&Vec<T>` params instead of `&str`/`&[T]`.
- `unsafe` without a `// SAFETY:` comment naming the invariant.
- Async: a `MutexGuard` held across `.await`; blocking I/O or CPU work on the executor (use `spawn_blocking`); a dropped `JoinHandle` losing a task's error.
- `pub` items that should be `pub(crate)`; new public API without doc comments.
<!-- /lang -->

### Error handling

- Swallowed errors: an empty handler, or one that returns a default as if it succeeded.
- Wrapped errors that lose context: a new generic error that discards the original cause.
- Handlers too broad, catching errors from unrelated code.
- Missing cases: 404, file not found, parse error.

<!-- lang:errors=rust -->
**Rust:**

- `Result<T, E>` for recoverable failure; `panic!` only for broken invariants. No `unwrap()`/`expect()` in production paths unless the invariant is proven and commented.
- Libraries: typed error enums (`thiserror`). Binaries: `anyhow` with `.context("loading config")?` so the chain says what failed.
- Silent failures: `let _ = fallible();`, `.ok()` dropping an error, `unwrap_or_default()`/`unwrap_or(0)` hiding a failure, `if let Ok(x) = …` with no `else`, `.map_err(|_| …)` discarding the source, ignored `#[must_use]` values, futures never awaited.
<!-- /lang -->

### Naming

- Names that lie: `isValid` returning a string, `getUser` that creates.
- Generic where a specific name exists: `data`, `result`, `temp`, `item`.
- Booleans missing an `is` / `has` / `should` prefix.
- Abbreviations that obscure: `usr`, `mgr` (idiomatic short names like Go's `ctx` are fine).

### Complexity & performance

- Functions over ~30 lines; nesting deeper than 3 levels (early returns flatten); more than 3 parameters.
- God functions doing read, validate, transform, persist, and notify.
- **Performance:** O(n²) iterations, accidental serialization, N+1 queries, unmemoized expensive work, unnecessary allocations, unbounded data structures.
- **Architecture:** leaky abstractions, broken separation of concerns, tight coupling, inconsistent error modeling, divergence from repository conventions, duplication where a helper already exists.

### Tests

- Changed behavior without a corresponding test change.
- Tests asserting implementation (mock call counts) instead of output values.
- Missing edge case for the specific code path that changed.

### What NOT to flag

- Style handled by linters and formatters.
- Minor naming preferences without clarity impact.
- "I would have done it differently" without a concrete problem.
- Types or docs for code outside the reviewed scope.
- Pre-existing issues outside the changed scope.

## Output

### Stage 1: Findings

Group by severity, most severe first. Each finding is 1–2 sentences — the problem and where, then
the fix — shaped per `caveman` and `i-have-adhd`:

- **[Silent Bug] path:line**: Cache key omits tenant ID, so concurrent lookups collide across tenants. Prepend `tenant_id` in the key builder.
- **[Edge Case] path:line**: Empty collection throws an unhandled exception. Add an early return for the empty state.
- **[Browser] /checkout, path:line**: Submit stays disabled after a valid card entry because the validity state never updates. Set it in the change handler.

If nothing substantial is wrong, say so plainly and stop — no menu.

### Stage 2: Fix plan & next steps

Write the fix plan (schema below) to `${TMPDIR:-/tmp}/qcheck-<scope-slug>.md` — never inside the
repository. Then offer, as a numbered list:

1. **Show the detailed fix plan** — print the plan file.
2. **Copy the fix plan as a handoff** — copy the plan file to the clipboard so another model can
   execute it (`pbcopy < file`, `wl-copy < file`, `xclip -selection clipboard < file`, or
   `clip.exe < file`, whichever exists). If none is available, render it in a code block instead.
3. **Execute the fixes** — run `/qcode`.

Fix plan schema (the same `EXECUTION HANDOFF` contract as `/qplan`, so `/qcode` runs it cold):

```plaintext
EXECUTION HANDOFF

Repository assumptions:
<branch, environment, paths, tools>

Task:
Fix the issues found reviewing <scope>

Required Rules & Skills:
- Rules: <rules to load before execution>
- Skills: <skills to load before execution>

Issues:
- Issue: <what is broken or suboptimal, path:line>
  Why: <risk, performance impact, or failure scenario>
  Fix: <how to resolve>

Detailed plan:
1. <ordered, actionable steps>

Validation:
<tests and checks to run>

Browser verification:
<the browser flows to re-run (URL, start command, flows, checks), or none>

Execution rules:
<scope boundaries — fix only the listed issues>
```
