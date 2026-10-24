---
paths:
  - "src/**/*.rs"
---

# Error Handling

- Use typed errors that carry a code or variant, not a generic "something went wrong".
- Never swallow errors silently. Log or rethrow with added context about what operation failed.
- Handle every failed async operation. No fire-and-forget calls whose errors go nowhere.
- HTTP error responses: consistent shape (`{ error: { code, message } }`), correct status codes (400 validation, 401 auth, 404 not found, 500 unexpected).
- Never expose stack traces, internal paths, or raw database errors in production responses.
- Retry transient errors (network timeouts, rate limits) with exponential backoff. Fail fast on validation and auth errors. Don't retry them.
- Include correlation or request IDs in error logs when available.

<!-- lang:errors=rust -->
**Rust:**

- `Result<T, E>` for recoverable failure; `panic!` only for broken invariants. No `unwrap()`/`expect()` in production paths unless the invariant is proven and commented.
- Libraries: typed error enums (`thiserror`). Binaries: `anyhow` with `.context("loading config")?` so the chain says what failed.
- Silent failures: `let _ = fallible();`, `.ok()` dropping an error, `unwrap_or_default()`/`unwrap_or(0)` hiding a failure, `if let Ok(x) = …` with no `else`, `.map_err(|_| …)` discarding the source, ignored `#[must_use]` values, futures never awaited.
<!-- /lang -->
