# Code Quality

## Anti-defaults (counter common Claude tendencies)

- No premature abstractions. Three similar lines beats a helper used once.
- Don't add features or improvements beyond what was asked.
- Don't refactor adjacent code while fixing a bug.
- No dead code or commented-out blocks. Git has history.
- WHY comments, never WHAT. If code needs a "what" comment, rename instead.
- API docs at module boundaries only, not every internal function.

## Naming

- Follow the language's own casing and API conventions. Names say what a thing is; booleans read as predicates (`is` / `has` / `should` / `can`).
- Abbreviations only when universally known (`id`, `url`, `api`, `db`, `auth`).

<!-- lang:naming=rust -->
**Rust:**

- `snake_case` functions, variables, modules, and files; `UpperCamelCase` types and traits; `SCREAMING_SNAKE_CASE` consts and statics. Acronyms as words: `HttpClient`, `user_id`.
- Conversions: `as_*` (cheap borrow), `to_*` (costly), `into_*` (consumes). Constructors `new`/`with_*`/`from_*`. Getters drop `get_` (`fn name(&self)`). Predicates `is_*`/`has_*`.
- `use` groups: `std`, external crates, then `crate::`/`super::`, blank line between. No glob imports outside tests and preludes.
- Items are `pub(crate)` by default; `pub` only for the intended API. Module files follow the repo (`foo.rs` vs `foo/mod.rs`).
<!-- /lang -->

## Code Markers

`TODO(author): desc (#issue)` for planned work. `FIXME(author): desc (#issue)` for known bugs. `HACK(author): desc (#issue)` for ugly workarounds (explain the proper fix). `NOTE: desc` for non-obvious context. Owner and issue link required. Never `XXX`, `TEMP`, `REMOVEME`.

## File Organization

- Function order: public API first, then helpers in call order.
- Group imports the way the language's formatter or linter does; never fight the formatter.
