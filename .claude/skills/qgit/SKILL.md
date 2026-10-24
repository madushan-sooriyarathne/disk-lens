---
name: qgit
description: Stage changes, run the project quality gate, and commit with a Conventional Commits message, then push on confirmation. Trigger with /qgit.
argument-hint: "[optional scope or message hint]"
disable-model-invocation: true
allowed-tools:
  - Read
  - Bash(git *)
  - Bash(gh *)
  - Agent
---

Take the working changes to a clean commit. Quality gate first, Conventional Commits message,
push only after confirmation.

## Steps

1. **Survey.** `git status` + `git diff` to see what changed. If nothing is staged or modified,
   say so and stop.
2. **Quality gate.** Run the project's checks in order, skipping any that don't exist: type-check,
   lint, test, build (read the manifest and the project instructions for the real command
   names). On failure, offer: fix and re-run / commit anyway / cancel — do not silently push
   broken code. When the gate has several commands or noisy output, and the host has a subagent
   tool that takes a model per call (Claude Code's `Agent` tool), run it in a `haiku` subagent.
   It reports pass or fail and only the failing lines, and never edits files. Otherwise run the
   gate inline.

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

3. **Stage** the relevant files (`git add`). Show the final file list.
4. **Compose the message** (see rules below) from the diff.
5. **Commit**, then show the hash and subject.
6. **Push** only after the user confirms. If the branch is the default branch (`main`/`master`),
   warn before pushing there directly.

## Commit message rules

- **MUST** use Conventional Commits: `<type>[optional scope]: <description>`.
- Types: `feat` `fix` `docs` `style` `refactor` `perf` `test` `build` `ci` `chore`.
- **MUST** use imperative mood ("add", not "added"); subject **≤ 72 chars**.
- Infer scope from the affected paths (e.g. `src/auth/` → `auth`) when it clarifies.
- Add a body only when the "why" isn't obvious from the subject.
- Breaking change → `type!:` subject and a `BREAKING CHANGE:` footer.
- **MUST NOT** reference the AI assistant, Claude, or Anthropic anywhere in the message.

## Output

Show the message before committing. Report gate results, the commit hash, and the push outcome.
