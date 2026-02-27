# Rust Workspace Agent Instructions

## Scope

- This template targets Rust workspaces only.
- `bin/` contains CLI binary crates.
- `crates/` contains reusable library crates.
- No frontend/web-framework-specific assumptions.

## Cargo Workspace Rules (Critical)

1. Never manually type dependency versions in `Cargo.toml`; use `cargo add`.
2. Add workspace-level dependencies with:

   ```bash
   cargo add <crate> --workspace
   ```

3. Add sub-crate dependencies with:

   ```bash
   cargo add <crate> -p <crate-name> --workspace
   ```

4. Root `[workspace.dependencies]` should use numeric versions only.
5. Root `[workspace.dependencies]` should not carry features by default.
6. Sub-crates must use `workspace = true` for `version`, `edition`, and shared dependencies.

## Preferred Dependencies and Versions

When introducing new dependencies, prefer these versions unless compatibility requires an upgrade:

- `clap = "4.5.60"`
- `config = "0.15.19"`
- `eyre = "0.6.12"`
- `serde = "1.0.228"`
- `thiserror = "2.0.18"`
- `tokio = "1.49.0"`
- `tracing = "0.1.44"`
- `tracing-subscriber = "0.3.22"`
- `tracing-opentelemetry = "0.32.1"`
- `opentelemetry = "0.31.0"`
- `opentelemetry-otlp = "0.31.0"`
- `sqlx = "=0.9.0-alpha.1"`
- `utoipa = "5.4.0"`
- `utoipa-swagger-ui = "9.0.2"`
- `arc-swap = "1.8.2"`
- `hpx = "2.3.1"`
- `scc = "3.6.5"`
- `winnow = "0.7.14"`
- `shadow-rs = "1.7.0"`
- `ecdysis = "1.0.1"`

## Dependency Priority and Forbidden Choices

- HTTP client preference: `hpx` (with `rustls`) over `reqwest`.
- Concurrent map/set preference: `scc` over `dashmap` and `RwLock<HashMap<...>>`.
- Parsing preference: `winnow` or `pest` over ad-hoc manual parsing.
- Read-heavy shared state: `arc-swap` over `RwLock`.
- Forbidden by default: `anyhow`, `log`, `reqwest`, `dashmap`.

## Engineering Principles

### Rust Implementation Guidelines

1. Error handling:
   - Application layer: `eyre`.
   - Library layer: `thiserror`.
2. Database (`sqlx`):
   - Prefer runtime queries (`sqlx::query_as`).
   - DB structs should derive `sqlx::FromRow`.
   - Avoid compile-time `sqlx::query!` macros by default.
3. Concurrency:
   - Prefer lock-free/container-first approaches (`scc`, `ArcSwap`).
   - Avoid `Arc<Mutex<T>>` when better alternatives are available.
4. Observability:
   - Logging: `tracing` only.
   - Metrics/traces: OpenTelemetry OTLP gRPC.
   - Prometheus should not be the default instrumentation path.
5. API docs:
   - Generate OpenAPI with `utoipa` when exposing HTTP APIs.
6. Configuration:
   - Use the `config` crate and external configuration files (prefer TOML).
7. Binaries:
   - Use `ecdysis` for graceful restart/reload flows in daemon/server binaries.
8. Safety:
   - Avoid `unsafe` unless strictly required and documented.

### Key Design Principles

- Modularity: Design each crate so it can be used as a standalone library with clear boundaries and minimal hidden coupling.
- Performance: Prefer architectures that support parallelism, memory-mapped I/O when appropriate, optimized data structures, and lock-free data types.
- Extensibility: Use traits and generic types to support multiple implementations without invasive refactors.
- Type Safety: Maintain strong static typing across interfaces and internals, with minimal use of dynamic dispatch.

### Performance Considerations

- Avoid allocations in hot paths; prefer references and borrowing to reduce allocation and copy overhead.
- Use `rayon` for CPU-bound parallel processing.
- Use `tokio` async/await for I/O-bound concurrency.

### Concurrency and Async Execution

- Prefer atomic types (`AtomicUsize`, `AtomicBool`, etc.) with explicit `Ordering` for simple shared state.
- Use `scc` for highly concurrent maps/sets; avoid `Arc<RwLock<HashMap<...>>>` and `Arc<Mutex<HashMap<...>>>` on hot paths.
- Use `moka` for concurrent caches instead of custom LRU implementations.
- Prefer `parking_lot::{Mutex, RwLock}` over `std::sync` locks for synchronous locking.
- Never hold `std::sync::Mutex` or `parking_lot::Mutex` guards across `.await`.
- Use `tokio::sync::Mutex` only when a lock must be held across `.await`.
- Use `tokio::task::spawn_blocking` for CPU-bound work and blocking I/O.
- Avoid massive volumes of tiny Tokio tasks; batch work or use bounded worker patterns.
- Channel selection:
  - Async-to-Async: `tokio::sync::mpsc` / `tokio::sync::broadcast`
  - Sync/MPMC: `crossbeam-channel` or `flume`
  - Avoid `std::sync::mpsc`

### Memory and Allocation

- For binary server applications, configure `tikv-jemallocator` or `mimalloc`.
- For trusted internal hash keys, prefer `ahash` or `rustc-hash` over default SipHash-based maps.
- Use `compact_str` or `smol_str` for small-string-heavy paths.
- Prefer `beef::Cow` over `std::borrow::Cow` when minimizing footprint.
- Use `bytes::Bytes` / `bytes::BytesMut` for network buffers; pass `Bytes` instead of cloning `Vec<u8>`.
- For critical serialization hot paths, prefer `rkyv` or `zerocopy`; reserve `serde_json` for config and non-critical APIs.

### Type and Layout

- Order struct fields from largest to smallest unless stronger semantic grouping is required.
- Use `#[repr(C)]` / `#[repr(packed)]` only for FFI or fixed protocol layout requirements.
- Keep error types compact on hot paths; box large error payloads when needed to reduce `Result<T, E>` size.
- Prefer typestate-style APIs for compile-time state transitions instead of runtime state checks.

### Tooling and Hot Paths

- Keep code clean under `clippy::pedantic`, `clippy::nursery`, and `clippy::cargo` (allow `missing_errors_doc` for non-public APIs when needed).
- Use `#[inline]` for tiny frequently called methods, especially across crate boundaries.
- Mark cold error paths with `#[cold]` and `#[inline(never)]` when it improves hot-path instruction locality.

### Common Pitfalls

- Do not block async tasks.
- Handle errors explicitly and consistently with the `?` operator and concrete error types.

### What to Avoid

- Incomplete implementations: finish features before submitting.
- Large, sweeping changes: keep changes focused and reviewable.
- Mixing unrelated changes: keep one logical change per commit.

## Foundry Rules (If Solidity Exists)

- Use `soldeer` for dependencies; do not use git submodules.
- Required commands:
  - `forge soldeer install`
  - `forge soldeer update`
  - `forge build`
  - `forge test`

## Development Workflow

When fixing failures, identify root cause first, then apply idiomatic fixes instead of suppressing warnings or patching symptoms.

After each feature or bug fix, run:

```bash
just format
just lint
just test
```

If any command fails, report the failure and do not claim completion.

## Testing Requirements

- Unit tests: colocate with implementation (`#[cfg(test)]`).
- Integration tests: place in crate-level `tests/`.
- Add tests for behavioral changes and public API changes.

## Language Requirement

- Documentation, comments, and commit messages must be English only.
