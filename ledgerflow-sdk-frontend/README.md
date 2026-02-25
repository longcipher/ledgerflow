# ledgerflow-sdk-frontend

Browser-compatible JavaScript/TypeScript bindings for the LedgerFlow Balancer API, compiled to WebAssembly via `wasm-pack`.

## Overview

This crate is a thin `wasm-bindgen` wrapper around `ledgerflow-sdk-rs` (with the `wasm` feature). It exposes the full LedgerFlow client API to JavaScript running in the browser.

> **Note:** This crate is excluded from the Cargo workspace because the `wasm32-unknown-unknown` target conflicts with native workspace builds. Build it standalone with `wasm-pack`.

## Prerequisites

```bash
# Install wasm-pack
cargo install wasm-pack
```

## Build

```bash
cd ledgerflow-sdk-frontend
wasm-pack build --target web
```

The output is placed in `pkg/` and contains:

- `ledgerflow_sdk_frontend_bg.wasm` — compiled WASM binary
- `ledgerflow_sdk_frontend.js` — JS glue code
- `ledgerflow_sdk_frontend.d.ts` — TypeScript declarations

## Usage

```js
import init, { LedgerFlowClient, generateOrderId } from "./pkg/ledgerflow_sdk_frontend.js";

await init();

const client = new LedgerFlowClient("https://api.ledgerflow.dev");

// Create an order
const order = await client.createOrder({
  account_id: 1,
  amount: "10.00",
  chain_id: 1,
});

// Fetch an order
const fetched = await client.getOrder(order.order_id);

// Register an account
const account = await client.registerAccount({
  username: "alice",
  email: "alice@example.com",
  telegram_id: 123456,
  evm_pk: "0xabc...",
});

// Look up accounts
const byName = await client.getAccountByUsername("alice");
const byEmail = await client.getAccountByEmail("alice@example.com");
const byTg = await client.getAccountByTelegramId(123456);

// Balance & admin
const balance = await client.getBalance(1);
const pending = await client.listPendingOrders(10, 0);

// Health check
const health = await client.healthCheck();

// Utility: deterministic order ID
const orderId = generateOrderId("broker-1", 1, 42);
```

## API

### `LedgerFlowClient`

| Method | Parameters | Returns |
|--------|-----------|---------|
| `new(baseUrl)` | `string` | `LedgerFlowClient` |
| `createOrder(request)` | `{ account_id, amount?, token_address?, chain_id?, broker_id? }` | `Promise<CreateOrderResponse>` |
| `getOrder(orderId)` | `string` | `Promise<OrderResponse>` |
| `registerAccount(request)` | `{ username, email, telegram_id, evm_pk, is_admin? }` | `Promise<RegisterAccountResponse>` |
| `getAccountByUsername(username)` | `string` | `Promise<AccountResponse>` |
| `getAccountByEmail(email)` | `string` | `Promise<AccountResponse>` |
| `getAccountByTelegramId(telegramId)` | `number` | `Promise<AccountResponse>` |
| `getBalance(accountId)` | `number` | `Promise<BalanceResponse>` |
| `listPendingOrders(limit?, offset?)` | `number?, number?` | `Promise<AdminOrdersResponse>` |
| `healthCheck()` | — | `Promise<HealthResponse>` |

### Standalone Functions

| Function | Parameters | Returns |
|----------|-----------|---------|
| `generateOrderId(brokerId, accountId, orderIdNum)` | `string, number, number` | `string` |

## Notes

- Numeric parameters that are `i64` in the Rust SDK are exposed as `f64` (JS `number`) on the WASM boundary. This is safe for all realistic values (< 2^53).
- Request/response objects are plain JS objects serialized via `serde-wasm-bindgen`.
- All async methods return native JS `Promise` objects.
