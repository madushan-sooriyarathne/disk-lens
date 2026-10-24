---
name: qnew
description: Start a coding session by discovering the project instruction files, resolving their precedence, and loading the skills they require before any work. Trigger with /qnew.
disable-model-invocation: true
allowed-tools:
  - Read
  - Glob
---

# QNew

Initialize the coding session by establishing the repository context, operating rules, and required skills before any planning or implementation occurs. Read, then commit — write no code yet.

## Initialization Workflow

1. **Locate workspace root.** Determine the current working directory and find the nearest version control root (`.git`).
2. **Discover instructions.** Search for instruction files from the root downward: `CLAUDE.md` (Claude Code) or `AGENTS.md` (Antigravity), plus any harness-specific directives.
3. **If none exists**, say so and stop — recommend running `/init` (or `/setup-agents`) to create
   one. Do not invent practices or start coding.
4. **Resolve precedence.**
   - Subdirectory rules override root-level rules.
   - Direct user instructions override repository instructions.
   - Project conventions stay authoritative unless explicitly contradicted.
   - If two instructions conflict and cannot be resolved, halt and ask the user.
5. **Load referenced skills.** Inspect the instructions for referenced skills. Read each relevant `SKILL.md` and apply its requirements. Do not load unrelated skills. Also load the session defaults `i-have-adhd`, `caveman`, and `ponytail` when they are installed — they are optional, so a missing one is skipped, not a failure.
6. **Read the rules.** Read any `.claude/rules/*.md` / `.agents/rules/*.md` and files under `guides/` or `docs/` the instruction file points to.
7. **Inspect the environment.** Identify the package manager, workspace layout (monorepo vs. standalone), build tools, lint rules, and test harnesses.
8. **Commit and confirm.** State that these govern every change this session, and name which session defaults actually loaded. Echo the key practices back in a short list (proof of comprehension, not a full reprint), shaped per any loaded defaults.

## Failure Conditions

Halt and ask the user for clarification if:

- A required instruction file cannot be parsed.
- The instruction file is empty or has no actionable rules — say exactly that and ask the user to fill it in before continuing.
- Instruction precedence is ambiguous and changes the task scope.
- A skill the instruction file requires is missing (the optional session defaults do not count).
- Critical instructions directly contradict each other.

## Output

Terse. `✓ Read <file> (+N rules) | Active: <loaded session defaults, or none>`, then a bulleted
list of the practices that will bind the session. End: `Ready.` Nothing more.
