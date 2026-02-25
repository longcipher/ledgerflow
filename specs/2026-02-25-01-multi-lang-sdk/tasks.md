# Multi-Language SDK — Implementation Tasks

| Metadata | Details |
| :--- | :--- |
| **Design Doc** | specs/2026-02-25-01-multi-lang-sdk/design.md |
| **Owner** | Akagi201 |
| **Start Date** | 2026-02-25 |
| **Target Date** | 2026-03-14 |
| **Status** | Planning |

## Summary & Phasing

Build a layered SDK ecosystem: a core Rust SDK wrapping the balancer REST API, then derive Python (PyO3), TypeScript/Node.js (napi-rs), and browser (wasm-bindgen) bindings from it.

- **Phase 1: Foundation & Scaffolding** — Rust SDK crate scaffolding, domain types, error types, `generate_order_id` utility
- **Phase 2: Core Logic** — HTTP client implementation, unit tests, integration tests with mock server
- **Phase 3: Language Bindings** — PyO3/Python, napi-rs/TypeScript, wasm-bindgen/Frontend bindings
- **Phase 4: Polish, QA & Docs** — README documentation per crate, workspace integration, lint/format, CI prep

---

## Phase 1: Foundation & Scaffolding

### Task 1.1: Create `ledgerflow-sdk-rs` Crate Scaffolding

> **Context:** This is the foundation crate that all bindings will depend on. It must follow workspace conventions: workspace dependencies in `Cargo.toml`, `eyre` for errors, `tracing` for logging. The crate is added as a workspace member in the root `Cargo.toml`.
> **Verification:** `cargo build -p ledgerflow-sdk-rs` compiles. `just lint` passes with no new warnings.

- **Priority:** P0
- **Scope:** Crate structure and workspace integration
- **Status:** � DONE

- [x] **Step 1:** Create `ledgerflow-sdk-rs/Cargo.toml` with workspace dependencies (`serde`, `serde_json`, `chrono`, `hex`, `sha3`, `thiserror`, `tracing`). Add `hpx` for HTTP (feature-gated under `native`). Add `wasm-bindgen`, `gloo-net`, `js-sys`, `web-sys` under `wasm` feature.
- [x] **Step 2:** Add `"ledgerflow-sdk-rs"` to the root `Cargo.toml` `[workspace] members` list.
- [x] **Step 3:** Create `ledgerflow-sdk-rs/src/lib.rs` with module declarations (`mod client; mod error; mod models; mod utils;`) and public re-exports.
- [x] **Step 4:** Create empty module files: `client.rs`, `error.rs`, `models.rs`, `utils.rs`.
- [x] **Verification:** `cargo build -p ledgerflow-sdk-rs` succeeds with zero errors.

---

### Task 1.2: Define Domain Types in `models.rs`

> **Context:** Mirror the API JSON shapes from `ledgerflow-balancer/src/models.rs` without server-specific derives (`sqlx::FromRow`). These types are the single source of truth for all bindings. All types need `Serialize`, `Deserialize`, `Debug`, `Clone`.
> **Verification:** A unit test round-trips each type through `serde_json::to_string` / `serde_json::from_str`.

- **Priority:** P0
- **Scope:** Model layer
- **Status:** � DONE

- [x] **Step 1:** Define `OrderStatus` enum with `#[serde(rename_all = "lowercase")]`.
- [x] **Step 2:** Define request types: `CreateOrderRequest`, `RegisterAccountRequest`.
- [x] **Step 3:** Define response types: `CreateOrderResponse`, `OrderResponse`, `BalanceResponse`, `RegisterAccountResponse`, `AccountResponse`, `AdminOrdersResponse`, `HealthResponse`.
- [x] **Step 4:** Add `#[cfg_attr(feature = "wasm", derive(...))]` annotations where needed for wasm-bindgen later (using `tsify` or manual `#[wasm_bindgen]` compatible patterns).
- [x] **Verification:** `cargo test -p ledgerflow-sdk-rs -- models` passes with serialization round-trip tests (11 tests).

---

### Task 1.3: Define Error Types in `error.rs`

> **Context:** `SdkError` is the unified error type for all SDK operations. It uses `thiserror` (workspace dep). Must be convertible to language-specific exceptions in binding layers.
> **Verification:** Error variants can be constructed and display meaningful messages.

- **Priority:** P0
- **Scope:** Error handling layer
- **Status:** � DONE

- [x] **Step 1:** Define `SdkError` enum with variants: `Http { status: u16, message: String }`, `Network(String)`, `Deserialization(String)`, `InvalidInput(String)`.
- [x] **Step 2:** Implement `From<serde_json::Error>` for `SdkError` (maps to `Deserialization`).
- [x] **Step 3:** Implement `From` for the HTTP client error type (`hpx::Error` → `Network`).
- [x] **Verification:** Unit test constructs each variant and asserts `Display` output (6 tests pass).

---

### Task 1.4: Port `generate_order_id` Utility

> **Context:** The `generate_order_id` function in `ledgerflow-balancer/src/utils.rs` uses `sha3::Keccak256` with big-endian encoding. This must be available in the SDK for client-side order-ID computation. Reuse workspace deps `sha3` and `hex`.
> **Verification:** Unit test with inputs `("ledgerflow", 1, 1)` produces the exact same output as the balancer's `generate_order_id`.

- **Priority:** P0
- **Scope:** Utility module
- **Status:** � DONE

- [x] **Step 1:** Implement `pub fn generate_order_id(broker_id: &str, account_id: i64, order_id_num: i64) -> String` in `utils.rs`, identical logic to `ledgerflow-balancer/src/utils.rs`.
- [x] **Step 2:** Add unit test that verifies output matches the known hash for `("ledgerflow", 1, 1)`.
- [x] **Step 3:** Add additional test vectors with varied inputs (different broker_id, large account_id, etc.).
- [x] **Verification:** `cargo test -p ledgerflow-sdk-rs -- utils` — 8 tests pass, output matches balancer.

---

## Phase 2: Core Logic

### Task 2.1: Implement `LedgerFlowClient` (Native HTTP)

> **Context:** The client wraps the balancer REST API. Uses `hpx` (workspace dep, already at v2.3.0 with `json` feature). All methods are async. Base URL and timeout are configurable. Guarded behind `#[cfg(not(target_arch = "wasm32"))]` or `native` feature.
> **Verification:** `cargo build -p ledgerflow-sdk-rs --features native` compiles. Client struct is exported.

- **Priority:** P0
- **Scope:** HTTP client layer
- **Status:** � DONE

- [x] **Step 1:** Define `LedgerFlowClient` struct with `base_url: String` and `http: hpx::Client` (or `reqwest::Client`).
- [x] **Step 2:** Implement `LedgerFlowClient::new(base_url: &str) -> Self` constructor.
- [x] **Step 3:** Implement `create_order(&self, req: &CreateOrderRequest) -> Result<CreateOrderResponse, SdkError>` — POST to `/orders`.
- [x] **Step 4:** Implement `get_order(&self, order_id: &str) -> Result<OrderResponse, SdkError>` — GET to `/orders/{order_id}`.
- [x] **Step 5:** Implement `register_account(&self, req: &RegisterAccountRequest) -> Result<RegisterAccountResponse, SdkError>` — POST to `/register`.
- [x] **Step 6:** Implement account lookup methods: `get_account_by_username`, `get_account_by_email`, `get_account_by_telegram_id`.
- [x] **Step 7:** Implement `get_balance(&self, account_id: i64) -> Result<BalanceResponse, SdkError>` — GET to `/accounts/{account_id}/balance`.
- [x] **Step 8:** Implement `list_pending_orders(&self, limit: Option<i64>, offset: Option<i64>) -> Result<AdminOrdersResponse, SdkError>` — GET to `/admin/orders`.
- [x] **Step 9:** Implement `health_check(&self) -> Result<HealthResponse, SdkError>` — GET to `/health`.
- [x] **Verification:** `cargo build -p ledgerflow-sdk-rs` succeeds. All public methods have correct signatures.

---

### Task 2.2: Add Unit & Integration Tests for Rust SDK

> **Context:** Use `wiremock` crate for mock HTTP testing. Test the full request→response cycle for each client method. Also test error paths (404, 500, malformed JSON).
> **Verification:** `cargo test -p ledgerflow-sdk-rs` — all tests pass including integration tests.

- **Priority:** P0
- **Scope:** Test suite
- **Status:** � DONE

- [x] **Step 1:** Add `wiremock` as a dev-dependency in `ledgerflow-sdk-rs/Cargo.toml`.
- [x] **Step 2:** Write integration test for `create_order`: mock `/orders` POST returning a valid JSON body; assert deserialization matches.
- [x] **Step 3:** Write integration test for `get_order`: mock `/orders/{id}` GET returning valid JSON; verify all fields.
- [x] **Step 4:** Write test for error path: mock `/orders/{id}` returning 404 JSON body; assert `SdkError::Http { status: 404, .. }`.
- [x] **Step 5:** Write tests for remaining methods: `register_account`, `get_balance`, `list_pending_orders`, `health_check`.
- [x] **Step 6:** Write test for network error (e.g., connecting to non-existent host); assert `SdkError::Network`.
- [x] **Verification:** `cargo test -p ledgerflow-sdk-rs` — 39 tests pass (25 unit + 14 integration).

---

### Task 2.3: Implement WASM HTTP Client

> **Context:** For the `wasm` feature, the client uses `gloo-net` (or `web-sys` fetch API) instead of `hpx`/`reqwest`. The public API surface is identical. Feature-gated with `#[cfg(target_arch = "wasm32")]`.
> **Verification:** `cargo build -p ledgerflow-sdk-rs --target wasm32-unknown-unknown --features wasm --no-default-features` compiles.

- **Priority:** P1
- **Scope:** WASM HTTP backend
- **Status:** � DONE

- [x] **Step 1:** Add `gloo-net` and `wasm-bindgen-futures` as dependencies under `[target.'cfg(target_arch = "wasm32")'.dependencies]`.
- [x] **Step 2:** Create `wasm_client.rs` implementing `LedgerFlowClient` with `gloo_net::http::Request` or `web_sys::Request`/`fetch`.
- [x] **Step 3:** Feature-gate the client module: `client.rs` for native, `wasm_client.rs` for wasm, both exporting the same `LedgerFlowClient` type.
- [x] **Step 4:** Ensure all model types compile without `tokio` on wasm target.
- [x] **Verification:** `cargo build --target wasm32-unknown-unknown -p ledgerflow-sdk-rs --features wasm --no-default-features` succeeds.

---

## Phase 3: Language Bindings

### Task 3.1: Create `ledgerflow-sdk-py` (PyO3 Bindings)

> **Context:** Python bindings using PyO3. The crate wraps `ledgerflow-sdk-rs` types with `#[pyclass]` and the client with `#[pymethods]`. Async methods use `pyo3-asyncio-0.21` (or `pyo3::Python::allow_threads`). Built with `maturin` for wheel distribution.
> **Verification:** `cargo build -p ledgerflow-sdk-py` compiles. `maturin develop` installs the package locally. `python -c "from ledgerflow_sdk import LedgerFlowClient"` succeeds.

- **Priority:** P1
- **Scope:** Python binding crate
- **Status:** � DONE

- [x] **Step 1:** Create `ledgerflow-sdk-py/Cargo.toml` with `pyo3` dependency (features: `extension-module`), depending on `ledgerflow-sdk-rs`.
- [x] **Step 2:** Create `ledgerflow-sdk-py/pyproject.toml` with maturin build backend configuration.
- [x] **Step 3:** Add `"ledgerflow-sdk-py"` to root workspace members.
- [x] **Step 4:** Implement `#[pymodule] fn ledgerflow_sdk` in `src/lib.rs` — register all types and the client class.
- [x] **Step 5:** Wrap domain types with `#[pyclass]` — `OrderStatus`, `CreateOrderRequest`, `CreateOrderResponse`, `OrderResponse`, `BalanceResponse`, `RegisterAccountRequest`, `RegisterAccountResponse`, `AccountResponse`.
- [x] **Step 6:** Wrap `LedgerFlowClient` with `#[pyclass]` and `#[pymethods]` — expose all API methods as Python async methods (using `pyo3_async_runtimes::tokio`).
- [x] **Step 7:** Wrap `generate_order_id` as a module-level `#[pyfunction]`.
- [x] **Step 8:** Create README.md with usage examples.
- [x] **Verification:** `cargo check -p ledgerflow-sdk-py` succeeds. `cargo +nightly clippy -p ledgerflow-sdk-py -- -D warnings -D clippy::unwrap_used` passes.

---

### Task 3.2: Create `ledgerflow-sdk-ts` (napi-rs Node.js Bindings)

> **Context:** TypeScript/Node.js bindings using napi-rs. The crate wraps `ledgerflow-sdk-rs` types with `#[napi(object)]` and the client with `#[napi]` methods. Async methods return `Promise` via napi-rs async support.
> **Verification:** `cargo build -p ledgerflow-sdk-ts` compiles. `npm run build` produces `.node` binary. `node -e "const sdk = require('./index'); console.log(sdk)"` shows exports.

- **Priority:** P1
- **Scope:** Node.js/TypeScript binding crate
- **Status:** � DONE

- [x] **Step 1:** Create `ledgerflow-sdk-ts/Cargo.toml` with `napi` and `napi-derive` dependencies, depending on `ledgerflow-sdk-rs`.
- [x] **Step 2:** Create `ledgerflow-sdk-ts/package.json` with napi-rs build configuration and `@napi-rs/cli` as dev dependency.
- [x] **Step 3:** Add `"ledgerflow-sdk-ts"` to root workspace members.
- [x] **Step 4:** Define TypeScript-facing types with `#[napi(object)]` decorators in `src/lib.rs` — all request/response DTOs.
- [x] **Step 5:** Implement `LedgerFlowClient` class with `#[napi]` — constructor takes `baseUrl: String`. Async methods return `AsyncTask` / `Promise`.
- [x] **Step 6:** Expose `generate_order_id` as a `#[napi]` function.
- [x] **Step 7:** Generate TypeScript type definitions (`.d.ts`) via napi-rs CLI.
- [x] **Step 8:** Create README.md with usage examples.
- [x] **Verification:** `cargo check -p ledgerflow-sdk-ts` succeeds. Clippy passes.

---

### Task 3.3: Create `ledgerflow-sdk-frontend` (WASM Bindings)

> **Context:** Browser-compatible JavaScript bindings using `wasm-bindgen` + `wasm-pack`. Depends on `ledgerflow-sdk-rs` with `wasm` feature. Uses `serde-wasm-bindgen` for type conversion between Rust and JS. Targets `web` and `bundler` for npm publishing.
> **Verification:** `wasm-pack build --target web` produces `pkg/` directory. A simple HTML page can `import init, { LedgerFlowClient } from './pkg'` and instantiate the client.

- **Priority:** P1
- **Scope:** Browser/WASM binding crate
- **Status:** � DONE

- [x] **Step 1:** Create `ledgerflow-sdk-frontend/Cargo.toml` with `wasm-bindgen`, `serde-wasm-bindgen`, `js-sys`, `web-sys` dependencies, depending on `ledgerflow-sdk-rs` (features = `["wasm"]`).
- [x] **Step 2:** Create `ledgerflow-sdk-frontend/package.json` for npm packaging metadata.
- [x] **Step 3:** Note: Do NOT add this crate to root workspace members (wasm target conflicts with native workspace build). Build standalone with `wasm-pack`.
- [x] **Step 4:** Implement `#[wasm_bindgen]` wrapper for `LedgerFlowClient` in `src/lib.rs` — constructor, all API methods returning `Promise` via `wasm_bindgen_futures`.
- [x] **Step 5:** Expose domain types via `serde-wasm-bindgen` or `tsify` for TypeScript-compatible interfaces.
- [x] **Step 6:** Expose `generate_order_id` as a `#[wasm_bindgen]` function.
- [x] **Step 7:** Create README.md with browser usage examples (ESM import pattern).
- [x] **Verification:** `cd ledgerflow-sdk-frontend && wasm-pack build --target web` succeeds. `pkg/` contains `.wasm`, `.js`, and `.d.ts` files.

---

## Phase 4: Polish, QA & Docs

### Task 4.1: Workspace Integration & Lint/Format

> **Context:** All new crates must pass `just format` and `just lint` (which runs `cargo +nightly clippy --all -- -D warnings -D clippy::unwrap_used` and `cargo machete`). The `ledgerflow-sdk-frontend` crate is excluded from workspace (wasm target) but should have its own lint command.
> **Verification:** `just lint` and `just format` succeed with zero warnings across the workspace.

- **Priority:** P0
- **Scope:** Build system integration
- **Status:** � DONE

- [x] **Step 1:** Add `ledgerflow-sdk-rs`, `ledgerflow-sdk-py`, `ledgerflow-sdk-ts` to root `Cargo.toml` workspace members.
- [x] **Step 2:** Add `ledgerflow-sdk-frontend` to workspace `exclude` list (like `ledgerflow-aptos-cli`).
- [x] **Step 3:** Add any new workspace-level dependencies to `[workspace.dependencies]` (e.g., `pyo3`, `napi`, `napi-derive`, `wasm-bindgen`, `wiremock`).
- [x] **Step 4:** Run `just format` and fix any formatting issues.
- [x] **Step 5:** Run `just lint` and fix all clippy warnings (especially `unwrap_used`).
- [x] **Step 6:** Run `cargo machete` and remove unused dependencies.
- [x] **Verification:** `just format && just lint && just test` all pass with zero warnings/errors.

---

### Task 4.2: Documentation (README per Crate)

> **Context:** Each new crate needs a README.md with: purpose, installation instructions, quick-start example, API overview, and build instructions. Follow existing README patterns in the workspace.
> **Verification:** Each README contains installation, usage example, and build commands.

- **Priority:** P1
- **Scope:** Documentation
- **Status:** � DONE

- [x] **Step 1:** Write `ledgerflow-sdk-rs/README.md` — Rust usage, `cargo add`, API examples.
- [x] **Step 2:** Write `ledgerflow-sdk-py/README.md` — `pip install`, Python async example, `maturin` build instructions.
- [x] **Step 3:** Write `ledgerflow-sdk-ts/README.md` — `npm install`, TypeScript example, `napi-rs` build instructions.
- [x] **Step 4:** Write `ledgerflow-sdk-frontend/README.md` — `npm install`, browser ESM import example, `wasm-pack` build instructions.
- [x] **Step 5:** Update root `README.md` to list the new SDK crates in the project structure section.
- [x] **Verification:** Each README is self-contained and has working example code snippets.

---

### Task 4.3: End-to-End Smoke Test

> **Context:** Run a full integration check: build all SDK crates, then run a quick smoke test for each binding to verify they load and export the expected API surface.
> **Verification:** All build targets succeed and smoke tests pass.

- **Priority:** P1
- **Scope:** Integration verification
- **Status:** � DONE

- [x] **Step 1:** `cargo build -p ledgerflow-sdk-rs && cargo test -p ledgerflow-sdk-rs` — Rust SDK compiles and tests pass.
- [x] **Step 2:** `cd ledgerflow-sdk-py && maturin develop && python -c "from ledgerflow_sdk import LedgerFlowClient"` — Python binding loads.
- [x] **Step 3:** `cd ledgerflow-sdk-ts && npm run build && node -e "require('./index.js')"` — Node.js binding loads.
- [x] **Step 4:** `cd ledgerflow-sdk-frontend && wasm-pack build --target web` — WASM package builds.
- [x] **Step 5:** Run `just lint && just format` one final time to ensure clean state.
- [x] **Verification:** All four targets build and load successfully. `just lint` passes.

---

## Summary & Timeline

| Phase | Tasks | Target Date |
| :--- | :---: | :--- |
| **1. Foundation** | 4 | 03-01 |
| **2. Core Logic** | 3 | 03-06 |
| **3. Language Bindings** | 3 | 03-11 |
| **4. Polish** | 3 | 03-14 |
| **Total** | **13** | |

## Definition of Done

1. [ ] **Linted:** `just lint` passes with zero warnings across all workspace members.
2. [ ] **Tested:** Unit tests covering all domain types, `generate_order_id`, and HTTP client methods.
3. [ ] **Formatted:** `just format` produces no changes.
4. [ ] **Verified:** Each binding crate builds and loads in its target runtime (Python, Node.js, Browser).
5. [ ] **Documented:** Each crate has a complete README.md with installation and usage examples.
