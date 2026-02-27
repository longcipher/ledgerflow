# ledgerflow-sdk-py

Python bindings for the LedgerFlow SDK, built with [PyO3](https://pyo3.rs/) and [maturin](https://www.maturin.rs/).

## Installation

```bash
# From source (requires Rust toolchain + maturin)
cd ledgerflow-sdk-py
pip install maturin
maturin develop
```

## Quick Start

```python
from ledgerflow_sdk import LedgerFlowClient, CreateOrderRequest, generate_order_id

# Create a client
client = LedgerFlowClient("https://api.ledgerflow.dev")

# Create an order
req = CreateOrderRequest(
    account_id=1,
    amount="10.00",
    token_address="0xA0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
    chain_id=1,
    broker_id="my-broker",
)
order = client.create_order(req)
print(f"Order: {order.order_id}, status: {order.status}")

# Generate order ID locally
order_id = generate_order_id("my-broker", 1, 42)
print(f"Order ID: {order_id}")
```

## Available Types

### Enums
- `OrderStatus` — `Pending`, `Deposited`, `Completed`, `Failed`, `Cancelled`

### Request Types
- `CreateOrderRequest(account_id, amount, token_address, chain_id, broker_id=None)`
- `RegisterAccountRequest(username, email, telegram_id, evm_address)`

### Response Types
- `CreateOrderResponse` — `order_id`, `amount`, `token_address`, `chain_id`, `status`, `created_at`
- `OrderResponse` — full order with timestamps and transaction hash
- `BalanceResponse` — `account_id`, `total_balance`, `completed_orders_count`
- `RegisterAccountResponse` — registered account details
- `AccountResponse` — account lookup result
- `AdminOrdersResponse` — `orders` (list), `total_count`
- `HealthResponse` — `status`, `timestamp`, `service`

### Client Methods
- `create_order(request)` → `CreateOrderResponse`
- `get_order(order_id)` → `OrderResponse`
- `register_account(request)` → `RegisterAccountResponse`
- `get_account_by_username(username)` → `AccountResponse`
- `get_account_by_email(email)` → `AccountResponse`
- `get_account_by_telegram_id(telegram_id)` → `AccountResponse`
- `get_balance(account_id)` → `BalanceResponse`
- `list_pending_orders(limit=None, offset=None)` → `AdminOrdersResponse`
- `health_check()` → `HealthResponse`

### Standalone Functions
- `generate_order_id(broker_id, account_id, order_id_num)` → `str`

## Building

```bash
# Development build
maturin develop

# Release wheel
maturin build --release
```

## License

Apache-2.0 OR MIT
