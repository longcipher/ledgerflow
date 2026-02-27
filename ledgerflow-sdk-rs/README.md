# ledgerflow-sdk-rs

Core Rust SDK for the LedgerFlow Balancer API. Provides typed HTTP client methods, domain models, error types, and the `generate_order_id` utility.

## Installation

```toml
[dependencies]
ledgerflow-sdk-rs = { path = "../ledgerflow-sdk-rs" }
```

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `native` | ✅ | Uses `hpx`/`tokio` for HTTP — standard desktop/server targets |
| `wasm` | | Uses `gloo-net` for HTTP — `wasm32-unknown-unknown` targets |

## Quick Start

```rust
use ledgerflow_sdk_rs::client::LedgerFlowClient;
use ledgerflow_sdk_rs::models::CreateOrderRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = LedgerFlowClient::new("https://api.ledgerflow.dev")?;

    // Create an order
    let order = client.create_order(&CreateOrderRequest {
        account_id: 1,
        amount: "10.00".into(),
        token_address: "0xA0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into(),
        chain_id: 1,
        broker_id: Some("my-broker".into()),
    }).await?;

    println!("Order created: {}", order.order_id);
    Ok(())
}
```

## API Methods

| Method | HTTP | Path |
|--------|------|------|
| `create_order` | POST | `/orders` |
| `get_order` | GET | `/orders/{id}` |
| `register_account` | POST | `/register` |
| `get_account_by_username` | GET | `/accounts/username/{username}` |
| `get_account_by_email` | GET | `/accounts/email/{email}` |
| `get_account_by_telegram_id` | GET | `/accounts/telegram/{id}` |
| `get_balance` | GET | `/accounts/{id}/balance` |
| `list_pending_orders` | GET | `/admin/orders` |
| `health_check` | GET | `/health` |

## Utilities

```rust
use ledgerflow_sdk_rs::utils::generate_order_id;

let order_id = generate_order_id("my-broker", 1, 42);
// Returns a 64-char hex string (keccak256 hash)
```

## Building

```bash
# Native
cargo build -p ledgerflow-sdk-rs

# WASM
cargo build -p ledgerflow-sdk-rs --target wasm32-unknown-unknown --features wasm --no-default-features

# Tests
cargo test -p ledgerflow-sdk-rs
```

## License

Apache-2.0 OR MIT
