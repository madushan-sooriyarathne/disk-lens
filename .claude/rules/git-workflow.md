# Git Workflow Rules

## Commit messages

Use Conventional Commits: `type(scope): subject`. Types: `feat`, `fix`, `refactor`, `perf`, `docs`, `test`, `chore`, `build`, `ci`, `style`. Scope is optional but encouraged and, in a monorepo, should name the affected package (`feat(web): ...`, `fix(api): ...`). Subject is imperative, lowercase, no trailing period, under ~72 characters. Put the _why_ in the body when it isn't obvious. One logical change per commit — don't bundle an unrelated refactor into a feature commit.

Breaking change: mark with `!` before the colon (`feat(api)!: drop v1 endpoints`) or a `BREAKING CHANGE: <description>` footer — either signals a MAJOR bump. Footers go one blank line after the body, as `Token: value` (hyphenate multi-word tokens, e.g. `Refs: PROJ-123`); `BREAKING CHANGE` is the one token kept uppercase.

## Branch naming

Branch off the default branch with a `type/short-description` name: `feat/invoice-export`, `fix/session-refresh`, `chore/bump-deps`. Use kebab-case. Include a ticket id when one exists: `feat/PROJ-123-invoice-export`.

## Never push directly to main

`main` is protected. All changes land via pull request and review. Never commit straight to `main`/`master` and never push to it directly, even for a "tiny" fix. If you find yourself on `main` with local changes, move them to a branch before pushing.

## Never bypass hooks

Do not use `git commit --no-verify` or `git push --no-verify`. Pre-commit and pre-push hooks (lint, type-check, tests, secret scanning) exist to catch problems before they land — bypassing them defeats the purpose. If a hook is failing, fix the underlying problem or fix the hook; do not skip it. The exception is genuinely broken hook tooling, and even then, flag it rather than silently bypassing.

## Never force-push destructively

Do not `git push --force`. If you must rewrite a branch you own (e.g. after a rebase), use `git push --force-with-lease`, which refuses to clobber commits you haven't seen. Never force-push a shared or protected branch.

## Before committing

Stage deliberately — review `git diff --staged`, don't blind `git add -A`. Keep secrets, `.env` files, build output, and local scratch files out of commits (they belong in `.gitignore`). Don't commit commented-out code or debug logging.
