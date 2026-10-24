---
paths:
  - "**/*.rs"
  - "Cargo.toml"
---

# Rust Rules

## Safety

Avoid `unsafe` unless it's genuinely required (FFI, low-level primitives the compiler can't verify). Every `unsafe` block needs a `// SAFETY:` comment immediately above it explaining which invariant makes it sound. Use `checked_*`/`saturating_*`/`wrapping_*` arithmetic on untrusted or user-supplied numbers — don't rely on default wrapping behavior where overflow is invalid. Use the newtype pattern (`struct EmailAddress(String)`) to make illegal states unrepresentable instead of passing raw primitives around.

## Error handling

`Result<T, E>` for anything recoverable; `panic!` only for unrecoverable invariant violations. No `unwrap()`/`expect()`/indexing-that-can-panic in production code paths — `expect()` is fine only in tests, init code where failure means the app can't run, or a structurally-guaranteed invariant the compiler can't see (with a comment explaining why). Libraries expose typed error enums (`thiserror`), never a bare `String` or `Box<dyn Error>` in a public signature. Application binaries can use `anyhow`/`eyre` with `?` for clean propagation.

## Idiomatic style

`cargo clippy --all-targets --all-features -- -D warnings` clean. Take `&str`/`&[T]`/`&Path` over `&String`/`&Vec<T>`/`&PathBuf` unless ownership is actually needed — don't clone to dodge a borrow-checker error. Implement `From<T>` for infallible conversions (gets `Into` for free); `TryFrom` when a conversion can fail. Prefer `match`/`if let`/iterator combinators over nested conditionals and manual loop indexing.

## Memory & concurrency

Pre-size collections with `with_capacity` when the size is known. Use `Cow` for conditional/zero-copy string handling. Don't reach for `Box`/`Rc`/`Arc` when a stack value or reference works — `Arc` only for cross-thread shared ownership, `Rc` for single-thread. Default to static dispatch (generics, `impl Trait`); use `dyn Trait` only for heterogeneous collections or when binary size matters more than speed. Keep mutex critical sections short — `drop(guard)` explicitly before long work. In async code (`tokio`), offload blocking/CPU-heavy work to `spawn_blocking` rather than stalling the executor.

## API design

Builder pattern for types with many optional construction params, instead of a constructor with a long parameter list. Keep internal fields `pub(crate)`, not `pub`, unless they're part of the intended public contract.

## Testing

Unit tests inline (`#[cfg(test)] mod tests`), integration tests in `tests/`, doc examples in `///` comments so they can't silently go stale.
