# LedgerFlow Agent Instructions

## Non-negotiables

- Surgical edits only: every changed line must trace to the request. No drive-by refactors, no reformatting untouched code, no deleting pre-existing dead code (mention it instead).
- Fix root causes. Never silence a warning, weaken a check, or patch a symptom to make a gate pass.
- Never report completion while any `just` gate below fails. Report the failure verbatim and stop.
- Stop and ask when a request is ambiguous, conflicts with existing conventions, or requires credentials, publishing, or destructive operations.
- When a correction reveals a missing rule, add one specific rule here ("always X for Y", never "be careful with Y"); delete rules that never catch real errors.

## Scope

- LedgerFlow is the payment-protocol plus authorization layer for AI-agent payments: x402 and MPP stay the merchant/agent wire protocols, and the LedgerFlow authz extension adds warrants, delegation chains, proof-of-possession, approval gates, revocation and trust anchors on top.
- `crates/ledgerflow-core` is the pure domain crate: warrant types and CBOR codec, delegation invariants I1-I7, PoP, constraints, m-of-n approvals, trust anchors, revocation seam, typestate builders. It performs no I/O, networking or storage; online checks are traits implemented downstream.
- `crates/ledgerflow-protocol` holds the x402/MPP extension codecs, the merchant verification middleware (with the `WarrantRepository` cache seam), replay protection and the payment-idempotency cache; `crates/ledgerflow-facilitator` verifies payments and routes them to settlement rails; `crates/ledgerflow-server` is the axum REST API (OpenAPI at `/openapi.json`, Swagger UI at `/swagger-ui`); `crates/ledgerflow-wallet` is the `WalletSigner` capability trait plus embedded and loopback JSON-RPC signers.
- `bin/ledgerflow-cli` produces deterministic fixtures; `bin/ledgerflow-server` (crate `ledgerflow-server-bin`) is the deployable binary.
- Design red line: no compile-time coupling to OneCipher or to the LongCipher platform. The wallet boundary is the `WalletSigner` trait over loopback JSON-RPC 2.0 (`ledgerflow_sign`, `ledgerflow_keys`, `ledgerflow_sign_payment`).
- `docs/design.md` is the design contract; `README.md` carries the architecture and the live-testnet results; `testnet-tests/` holds reusable live-network scripts.
- English only in code, comments, docs, and commit messages.

## Execution

- Parallelize aggressively: dispatch subagents for independent research, exploration, and implementation.
- NEVER run two `cargo` commands concurrently, and never overlap file edits with a running `cargo` command (`target/` file lock): finish edits, wait, then run.
- NEVER use `git worktree`. Edit the current branch in place.
- TDD for behavior changes: failing test first, smallest change to pass, then run the mutation gate only when the current spec task declares `Mutation:`, and fix surviving mutants.

## Verification loop

After each feature or bug fix, in order; any failure means the task is not done:

```bash
just format
just lint
just test
```

Mutation testing is intentionally absent from the default loop and from CI. When a spec task
declares `Mutation: <src-glob>`, follow the `/pb-build` gate: write the changed file's diff to a
temp file and run `cargo mutants --in-diff <diff-file>` for that file only, with at most 3 files /
60 mutants / 300 s. Never add a whole-repo mutation recipe: `just check-mutation-scope` fails
`just lint` when an unbounded mutation command appears in a build or CI file.

`just lint` is exactly the tool set CI installs: `typos`, `rumdl`, `cargo sort -w -g -c`, `cargo +nightly fmt --check`, `cargo +nightly clippy --all -- -D warnings`, Cargo's native `unused_dependencies` lint. `just mutate` is the legacy alias of the mutation recipe.

## Workspace rules

- Never hand-type dependency versions. Add per-crate deps with `cargo add <crate> -p <crate>`. Shared deps go into root `[workspace.dependencies]` (numeric versions only, no default features) and are referenced with `dep = { workspace = true }`.
- Sub-crates inherit `version`/`edition`/`license`/`repository` via `workspace = true` and must set `[lints] workspace = true`.
- There is no `rust-toolchain.toml`; CI installs a nightly toolchain, so use `cargo +nightly fmt` and `cargo +nightly clippy` exactly as the Justfile does.
- Clippy is `pedantic` plus a curated nursery set. `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `dbg_macro`, `print_stdout` and `print_stderr` are denied. `allow_attributes` is allowed, and the tree uses plain `#[allow(...)]` (never `#[expect]`), always naming the lint.
- Every library crate opts out of `missing_docs` at crate level (`#![allow(missing_docs)]`), so public items are documented by convention, not enforced by the compiler.
- Mutation testing is scoped to `ledgerflow-core`; the `mutants` dev-dependency needs no ignore entry, because Cargo's native `unused_dependencies` lint does not check dev-dependencies.
- Fuzz targets live in `crates/ledgerflow-core/fuzz` and `crates/ledgerflow-protocol/fuzz` as separate cargo-fuzz workspaces: `just fuzz-check` type-checks them, `just fuzz-smoke` runs one-second passes. Never add a third fuzz workspace without a real decoder to target.
- Workspace MSRV is Rust 1.100. Prefer newly stabilized features and APIs over older workarounds; do not add feature gates for anything already stable in 1.100.
- `[workspace.lints.cargo]` enables Cargo's native `unused_dependencies` lint, the only unused-dependency check; do not reintroduce a third-party checker. Its level is `warn`, and `.cargo/config.toml` sets `build.warnings = "deny"`, so a finding fails the gate.
- `.cargo/config.toml` sets resolver `incompatible-rust-versions = "fallback"` (MSRV-aligned resolution) and `registry.global-min-publish-age = "5 days"` (Rust 1.100 dependency cooldown); keep both.

## Dependency choices

- HTTP: `hpx` with rustls, never `reqwest`. WalletConnect v2 relay: `hpx-yawc`.
- Errors: `thiserror` in `crates/`, `eyre` in `bin/`. Never `anyhow`.
- Observability: `tracing` only. There is no `log` crate.
- Sync locking: `parking_lot`. Async coordination: `flume` (the server's bounded webhook queue) or `tokio::sync`. Never `std::sync::mpsc`.
- Crypto: `ed25519-dalek` (warrants), `k256` plus `tiny-keccak` (EVM, ERC-1271), `x25519-dalek` with `chacha20poly1305` and `hkdf` (WalletConnect v2 envelopes), `sha2`, `subtle` (constant-time compares), `zeroize`.
- No `sqlx`: this workspace has no database. No `dashmap`, no `arc-swap` (the only cache is `lru` behind `parking_lot`).

## Code conventions

- Warrant verification is a typestate pipeline (`WarrantBuilder`, `verify_*`): keep domain crates I/O-free and express online checks (revocation, budget accounting) as traits.
- Fail closed. An unverifiable warrant, a broken delegation chain or an unavailable revocation lookup is a rejection, never a default-allow.
- Revocation is an online security commitment: production MUST persist the revocation store. In-memory mode is demo-only and must stay behind the explicit `--insecure-revoc-memory` flag.
- `caip10` subjects route by chain family: `caip10:eip155:...` resolves to the EVM adapter, `caip10:solana:...` to the Solana adapter, in both the Facilitator and the server runtime.
- Server configuration is environment-driven and fail-fast: `LEDGERFLOW_BIND`, `LEDGERFLOW_SAAS_MODE`, `LEDGERFLOW_SERVICE_TOKEN`, `LEDGERFLOW_TENANT_ID`, `LEDGERFLOW_ISSUER_KEY`, `LEDGERFLOW_WEBHOOK_URL`. `LEDGERFLOW_ISSUER_KEY` is required and never defaults to a demo key.
- In `saas` mode every request must carry the gateway-injected internal headers validated against `LEDGERFLOW_SERVICE_TOKEN`; revocation and audit views are tenant-scoped.
- Webhook delivery is best-effort and in-process but runs through one bounded background worker queue, never one thread per emitted event.
- Replay protection combines `challenge_id` + `nonce` fingerprinting with payment-identifier idempotency. Keep both halves.
- Tests live in `crates/*/tests/` plus colocated `#[cfg(test)]` modules; business cases are example-based tests, and there is currently no property-test framework in this workspace.
