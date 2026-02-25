# Design Document: Multi-Language SDK

| Metadata | Details |
| :--- | :--- |
| **Author** | pb-plan agent |
| **Status** | Draft |
| **Created** | 2026-02-25 |
| **Reviewers** | Akagi201 |
| **Related Issues** | N/A |

## 1. Executive Summary

**Problem:** LedgerFlow currently exposes its functionality only through the REST API served by `ledgerflow-balancer` and smart contract interactions. Third-party developers and internal services have no ergonomic, type-safe client library for integrating with the system. Each consumer must hand-craft HTTP requests, duplicate data types, and re-implement error handling.

**Solution:** Introduce a layered SDK ecosystem — a **core Rust SDK** (`ledgerflow-sdk-rs`) that encapsulates API models, HTTP client logic, and order-ID generation, then derive language bindings automatically:

- `ledgerflow-sdk-py` — Python bindings via **PyO3**
- `ledgerflow-sdk-ts` — Node.js/TypeScript bindings via **napi-rs**
- `ledgerflow-sdk-frontend` — Browser/frontend JS bindings via **wasm-bindgen + wasm-pack**

---

## 2. Requirements & Goals

### 2.1 Problem Statement

Integrators today must:

1. Manually construct HTTP requests to the balancer API (create order, get order, register account, query balance, list orders).
2. Duplicate data types (`Order`, `OrderStatus`, `Account`, `Balance`, request/response structs) in their language of choice.
3. Re-implement `generate_order_id` (keccak256-based) to produce correct order IDs client-side.
4. Handle pagination, error mapping, and authentication on their own.

This friction increases integration time and creates inconsistency across consumers.

### 2.2 Functional Goals

1. **FG-1:** Provide a native Rust SDK crate (`ledgerflow-sdk-rs`) that wraps the ledgerflow-balancer REST API with typed methods: `create_order`, `get_order`, `register_account`, `get_balance`, `list_pending_orders`, `health_check`.
2. **FG-2:** Re-export core domain types (`Order`, `OrderStatus`, `Account`, `Balance`, request/response DTOs) from the SDK so consumers do not duplicate them.
3. **FG-3:** Expose the `generate_order_id` utility in the SDK for client-side order-ID computation.
4. **FG-4:** Provide Python bindings (`ledgerflow-sdk-py`) built with PyO3, published as a pip-installable wheel.
5. **FG-5:** Provide TypeScript/Node.js bindings (`ledgerflow-sdk-ts`) built with napi-rs, published as an npm package.
6. **FG-6:** Provide browser-compatible JS bindings (`ledgerflow-sdk-frontend`) built with wasm-bindgen + wasm-pack, published as an npm package (pkg target: `bundler` & `web`).

### 2.3 Non-Functional Goals

- **Performance:** The Rust SDK must use async HTTP (`hpx`/`reqwest`) and compile to efficient native code. WASM bundle should be < 500 KB gzipped.
- **Reliability:** All SDK methods must propagate structured errors (network, HTTP status, deserialization) without panicking.
- **Security:** No secrets stored in the SDK. Base URL is configurable. HTTPS enforced by default.
- **Ergonomics:** Each language binding should feel idiomatic (Python: snake_case, async with `asyncio`; TypeScript: camelCase, Promise-based; Browser: same as TS but WASM-backed).
- **CI/CD:** Each binding package must be buildable and testable in CI (GitHub Actions).

### 2.4 Out of Scope

- Authentication middleware (the balancer API currently has no auth layer; authentication will be a separate effort).
- Smart-contract interaction helpers (those belong in the existing CLI tools or a future on-chain SDK).
- x402 facilitator API wrappers (the facilitator has its own protocol; not part of this SDK scope).
- WebSocket/streaming APIs.
- Mobile bindings (Swift/Kotlin).

### 2.5 Assumptions

- The balancer REST API is the primary integration surface; the SDK wraps its existing endpoints (`/orders`, `/register`, `/accounts/*`, `/admin/orders`, `/health`).
- `hpx` (the existing HTTP client in the workspace) will be used for the Rust SDK HTTP layer for consistency; if it lacks required features, `reqwest` will be added as a workspace dependency.
- napi-rs v3 is used (latest stable).
- PyO3 v0.24+ is used (latest stable).
- wasm-pack v0.13+ targets `bundler` and `web`.
- The SDK crates live in the monorepo as workspace members.

---

## 3. Architecture Overview

### 3.1 System Context

```
┌─────────────────────────────────────────────────────┐
│                  External Consumers                  │
│  (Python apps, Node.js services, Browsers, Rust)     │
└────────┬──────────┬──────────┬──────────┬───────────┘
         │          │          │          │
  ┌──────▼──┐ ┌─────▼────┐ ┌──▼─────┐ ┌──▼──────────┐
  │sdk-rs   │ │sdk-py    │ │sdk-ts  │ │sdk-frontend │
  │(Rust)   │ │(PyO3)    │ │(napi)  │ │(WASM)       │
  └────┬────┘ └────┬─────┘ └───┬────┘ └─────┬───────┘
       │           │           │             │
       │     ┌─────▼─────┐    │             │
       │     │sdk-rs core│◄───┘             │
       │     │(lib)      │◄─────────────────┘
       └─────►           │
             └─────┬─────┘
                   │ HTTP
             ┌─────▼─────────────┐
             │ ledgerflow-balancer│
             │ REST API           │
             └───────────────────┘
```

All language bindings depend on `ledgerflow-sdk-rs` (the core crate). The core crate has two feature flags:

- `native` (default) — uses `hpx`/`reqwest` with tokio for async HTTP.
- `wasm` — uses `gloo-net` / `web-sys` fetch for browser environments; disables tokio runtime.

### 3.2 Key Design Principles

1. **Single source of truth:** All domain types and API logic live in `ledgerflow-sdk-rs`. Bindings only wrap/re-export.
2. **Feature-gated platform support:** `native` vs `wasm` feature controls the HTTP backend. Types and logic are shared.
3. **Idiomatic bindings:** Each binding layer translates Rust types to language-native equivalents (e.g., PyO3 `#[pyclass]`, napi-rs `#[napi]`, wasm-bindgen `#[wasm_bindgen]`).
4. **No code duplication:** Request/response types defined once in Rust; mapped via derive macros in binding layers.
5. **Workspace conventions:** All new crates follow workspace dependency patterns, use `eyre` for errors, `tracing` for logging, and adhere to `just format`/`just lint`.

### 3.3 Existing Components to Reuse

| Component | Location | How to Reuse |
| :--- | :--- | :--- |
| Domain models (`Order`, `OrderStatus`, `Account`, `Balance`, all request/response DTOs) | `ledgerflow-balancer/src/models.rs` | Extract and re-define in `ledgerflow-sdk-rs` (without `sqlx::FromRow`/server-specific derives). The SDK types mirror the API JSON shapes. |
| `generate_order_id` | `ledgerflow-balancer/src/utils.rs` | Move the keccak256 logic into `ledgerflow-sdk-rs` so both server and clients can use it. The balancer can depend on the SDK crate for this function. |
| Workspace dependency versions | Root `Cargo.toml` `[workspace.dependencies]` | All new crates reference workspace deps (`serde`, `chrono`, `hex`, `sha3`, `thiserror`, `eyre`, `tracing`). |
| `hpx` HTTP client | Workspace dep `hpx = "2.3.0"` | Use as the HTTP layer in `ledgerflow-sdk-rs` for `native` feature. |
| Build/lint toolchain | `Justfile` (`just format`, `just lint`) | All new crates must pass `just lint` with zero warnings. |

---

## 4. Detailed Design

### 4.1 Module Structure

```
ledgerflow-sdk-rs/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs            # Re-exports
│   ├── client.rs         # LedgerFlowClient — HTTP wrapper
│   ├── models.rs         # Shared domain types (Order, Account, etc.)
│   ├── error.rs          # SdkError enum
│   ├── utils.rs          # generate_order_id, helpers
│   └── wasm_client.rs    # WASM-specific HTTP client (feature = "wasm")

ledgerflow-sdk-py/
├── Cargo.toml
├── pyproject.toml
├── README.md
├── src/
│   └── lib.rs            # PyO3 #[pymodule] wrapping sdk-rs types & client

ledgerflow-sdk-ts/
├── Cargo.toml
├── package.json
├── README.md
├── src/
│   └── lib.rs            # napi-rs #[napi] exports

ledgerflow-sdk-frontend/
├── Cargo.toml
├── package.json
├── README.md
├── src/
│   └── lib.rs            # wasm_bindgen exports
```

### 4.2 Data Structures & Types

Core types in `ledgerflow-sdk-rs/src/models.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Deposited,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub account_id: i64,
    pub amount: Option<String>,
    pub token_address: Option<String>,
    pub chain_id: Option<i64>,
    pub broker_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderResponse {
    pub order_id: String,
    pub amount: Option<String>,
    pub token_address: Option<String>,
    pub chain_id: Option<i64>,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order_id: String,
    pub account_id: i64,
    pub amount: String,
    pub token_address: String,
    pub chain_id: i64,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub transaction_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub account_id: i64,
    pub total_balance: String,
    pub completed_orders_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAccountRequest {
    pub username: String,
    pub email: String,
    pub telegram_id: i64,
    pub evm_pk: String,
    pub is_admin: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAccountResponse {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub telegram_id: Option<i64>,
    pub evm_address: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountResponse {
    pub id: i64,
    pub username: String,
    pub email: Option<String>,
    pub telegram_id: Option<i64>,
    pub evm_address: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminOrdersResponse {
    pub orders: Vec<OrderResponse>,
    pub total_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub service: String,
}
```

### 4.3 Interface Design

**Rust Client (`ledgerflow-sdk-rs`):**

```rust
pub struct LedgerFlowClient {
    base_url: String,
    http: hpx::Client, // or reqwest::Client
}

impl LedgerFlowClient {
    pub fn new(base_url: &str) -> Self;

    // Orders
    pub async fn create_order(&self, req: &CreateOrderRequest) -> Result<CreateOrderResponse, SdkError>;
    pub async fn get_order(&self, order_id: &str) -> Result<OrderResponse, SdkError>;
    pub async fn list_pending_orders(&self, limit: Option<i64>, offset: Option<i64>) -> Result<AdminOrdersResponse, SdkError>;

    // Accounts
    pub async fn register_account(&self, req: &RegisterAccountRequest) -> Result<RegisterAccountResponse, SdkError>;
    pub async fn get_account_by_username(&self, username: &str) -> Result<AccountResponse, SdkError>;
    pub async fn get_account_by_email(&self, email: &str) -> Result<AccountResponse, SdkError>;
    pub async fn get_account_by_telegram_id(&self, telegram_id: i64) -> Result<AccountResponse, SdkError>;

    // Balance
    pub async fn get_balance(&self, account_id: i64) -> Result<BalanceResponse, SdkError>;

    // Health
    pub async fn health_check(&self) -> Result<HealthResponse, SdkError>;
}

// Utility (no client needed)
pub fn generate_order_id(broker_id: &str, account_id: i64, order_id_num: i64) -> String;
```

**Python API (`ledgerflow-sdk-py`):**

```python
from ledgerflow_sdk import LedgerFlowClient, CreateOrderRequest, OrderStatus

client = LedgerFlowClient("https://api.ledgerflow.example")
order = await client.create_order(CreateOrderRequest(account_id=1, amount="100"))
```

**TypeScript API (`ledgerflow-sdk-ts`):**

```typescript
import { LedgerFlowClient, CreateOrderRequest } from 'ledgerflow-sdk';

const client = new LedgerFlowClient('https://api.ledgerflow.example');
const order = await client.createOrder({ accountId: 1, amount: '100' });
```

**Frontend JS API (`ledgerflow-sdk-frontend`):**

```javascript
import init, { LedgerFlowClient } from 'ledgerflow-sdk-frontend';

await init();
const client = new LedgerFlowClient('https://api.ledgerflow.example');
const order = await client.createOrder({ accountId: 1, amount: '100' });
```

### 4.4 Logic Flow

1. Consumer creates `LedgerFlowClient` with a base URL.
2. Consumer calls a typed method (e.g., `create_order`).
3. The SDK serializes the request to JSON, sends HTTP POST/GET to the balancer endpoint.
4. The SDK deserializes the JSON response into the typed response struct.
5. On HTTP error (4xx/5xx), the SDK parses the error body `{ error, message, status }` and returns a structured `SdkError`.
6. For `generate_order_id`, computation is purely local (keccak256).

### 4.5 Configuration

| Config | Description | Default |
| :--- | :--- | :--- |
| `base_url` | Balancer API base URL | Required (no default) |
| `timeout` | HTTP request timeout | 30 seconds |

No YAML config for the SDK. Configuration is programmatic via the client constructor.

### 4.6 Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    #[error("HTTP error: {status} - {message}")]
    Http { status: u16, message: String },

    #[error("Network error: {0}")]
    Network(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}
```

Each binding translates `SdkError` to language-idiomatic exceptions/errors:
- Python: raises `LedgerFlowError` (with `status_code`, `message` attributes)
- TypeScript: throws `LedgerFlowError` extending `Error`
- WASM: returns `JsValue` error via `wasm_bindgen`

---

## 5. Verification & Testing Strategy

### 5.1 Unit Testing

- `ledgerflow-sdk-rs`: Test `generate_order_id` output against known vectors from `ledgerflow-balancer/src/utils.rs`. Test model serialization/deserialization round-trips.
- `ledgerflow-sdk-py`: PyO3 test using `pyo3::prepare_freethreaded_python()` for type conversion checks.
- `ledgerflow-sdk-ts`: napi-rs built-in test harness for type mapping.

### 5.2 Integration Testing

- Mock HTTP server (e.g., `wiremock` crate) to simulate balancer responses.
- `ledgerflow-sdk-rs` integration tests call the mock server to verify full request/response cycle.
- Python/TS integration tests use the same mock server pattern.

### 5.3 Critical Path Verification (The "Harness")

| Verification Step | Command | Success Criteria |
| :--- | :--- | :--- |
| **VP-01** | `cargo build -p ledgerflow-sdk-rs` | Compiles with zero warnings |
| **VP-02** | `cargo test -p ledgerflow-sdk-rs` | All tests pass |
| **VP-03** | `cargo build -p ledgerflow-sdk-py` | PyO3 module compiles |
| **VP-04** | `cargo build -p ledgerflow-sdk-ts` | napi-rs module compiles |
| **VP-05** | `cd ledgerflow-sdk-frontend && wasm-pack build --target web` | WASM package builds |
| **VP-06** | `just lint` | Zero warnings across all workspace members |
| **VP-07** | `just format` | No formatting changes needed |

### 5.4 Validation Rules

| Test Case ID | Action | Expected Outcome | Verification Method |
| :--- | :--- | :--- | :--- |
| **TC-01** | Call `generate_order_id("ledgerflow", 1, 1)` in Rust SDK | Output matches balancer's `generate_order_id` with same inputs | Unit test with known hash |
| **TC-02** | Call `client.create_order(...)` against mock server | Returns deserialized `CreateOrderResponse` | Integration test |
| **TC-03** | Call `client.get_order("nonexistent")` against mock returning 404 | Returns `SdkError::Http { status: 404, .. }` | Integration test |
| **TC-04** | Import `ledgerflow_sdk` in Python, instantiate client | No import errors, class available | Smoke test |
| **TC-05** | `require('ledgerflow-sdk')` in Node.js | Module loads, exports available | Smoke test |
| **TC-06** | Load WASM in browser test harness | `init()` succeeds, client instantiable | wasm-pack test |

---

## 6. Implementation Plan

- [ ] **Phase 1: Foundation** — Create `ledgerflow-sdk-rs` crate with types, error module, and `generate_order_id`.
- [ ] **Phase 2: Core Logic** — Implement `LedgerFlowClient` HTTP wrapper with all API methods; add unit & integration tests.
- [ ] **Phase 3: Language Bindings** — Build `ledgerflow-sdk-py` (PyO3), `ledgerflow-sdk-ts` (napi-rs), `ledgerflow-sdk-frontend` (wasm-bindgen).
- [ ] **Phase 4: Polish** — Documentation (README per crate), CI configuration, `just lint`/`just format`, workspace integration.

---

## 7. Cross-Functional Concerns

- **Backward Compatibility:** The SDK is additive — no changes to the existing balancer API. The balancer may optionally depend on `ledgerflow-sdk-rs` for shared types and `generate_order_id`, but this is a Phase 4 refactor opportunity, not a requirement.
- **Versioning:** All SDK crates start at `0.1.0`. Follow semver. The Rust crate version drives binding package versions.
- **CI/CD:** GitHub Actions workflows needed for: (1) Rust SDK build+test, (2) maturin build for Python wheel, (3) napi-rs build for npm, (4) wasm-pack build. Not implemented in this spec — separate CI task.
- **Security:** The SDK transmits data over HTTP(S) to the balancer. No secrets stored in the SDK. Users are responsible for securing their base URL (HTTPS).
- **Monitoring:** The SDK produces `tracing` spans for each API call (Rust only). Bindings surface errors via language-native mechanisms.
