# ledgerflow-sdk-ts

Node.js/TypeScript bindings for the LedgerFlow SDK, built with [napi-rs](https://napi.rs/).

## Installation

```bash
# From source (requires Rust toolchain + @napi-rs/cli)
cd ledgerflow-sdk-ts
npm install
npm run build
```

## Quick Start

```typescript
import { LedgerFlowClient, generateOrderId } from '@ledgerflow/sdk';

// Create a client
const client = new LedgerFlowClient('https://api.ledgerflow.dev');

// Create an order
const order = await client.createOrder({
  accountId: 1,
  amount: '10.00',
  brokerId: 'my-broker',
});
console.log(`Order: ${order.orderId}, status: ${order.status}`);

// Generate order ID locally
const orderId = generateOrderId('my-broker', 1n, 42n);
console.log(`Order ID: ${orderId}`);
```

## API

### `LedgerFlowClient`

```typescript
new LedgerFlowClient(baseUrl: string)
```

#### Methods (all return `Promise`)

| Method | Parameters | Returns |
|--------|-----------|---------|
| `createOrder(request)` | `CreateOrderRequest` | `CreateOrderResponse` |
| `getOrder(orderId)` | `string` | `OrderResponse` |
| `registerAccount(request)` | `RegisterAccountRequest` | `RegisterAccountResponse` |
| `getAccountByUsername(username)` | `string` | `AccountResponse` |
| `getAccountByEmail(email)` | `string` | `AccountResponse` |
| `getAccountByTelegramId(telegramId)` | `bigint` | `AccountResponse` |
| `getBalance(accountId)` | `bigint` | `BalanceResponse` |
| `listPendingOrders(limit?, offset?)` | `number?, number?` | `AdminOrdersResponse` |
| `healthCheck()` | — | `HealthResponse` |

### `generateOrderId(brokerId, accountId, orderIdNum)`

Standalone function that computes a keccak256 order ID matching the on-chain logic.

## Types

All response types expose fields as plain JavaScript objects. `OrderStatus` is represented as a lowercase string (`"pending"`, `"deposited"`, `"completed"`, `"failed"`, `"cancelled"`). Timestamps are ISO 8601 strings.

## Building

```bash
# Debug build
npm run build:debug

# Release build
npm run build
```

## License

Apache-2.0 OR MIT
