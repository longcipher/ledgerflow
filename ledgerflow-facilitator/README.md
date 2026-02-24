# LedgerFlow Facilitator (x402 v2)

LedgerFlow facilitator now uses a chain-agnostic x402 v2 architecture with an adapter registry.

## Highlights

- x402 v2 request/response model
- Chain-agnostic network IDs (`namespace:reference`)
- Adapter registry keyed by `(x402Version, scheme, network pattern)`
- Built-in offchain/CEX adapter (`mock` + `http` backend)
- Simple integration model for centralized payment systems

## Endpoints

- `GET /supported`
- `POST /verify`
- `POST /settle`
- `GET /health`

## Quick Start

```bash
cd ledgerflow-facilitator
cp config.example.toml config.toml
cargo run
```

Default server address: `0.0.0.0:3402`

## Config Example

```toml
host = "0.0.0.0"
port = 3402

[[adapters]]
kind = "offchain"
id = "cex-mock"
x402_version = 2
scheme = "exact"
networks = ["offchain:*"]

[adapters.backend]
type = "mock"
payer = "cex:user:alice"
transaction_prefix = "cex-tx"
```

### Production CEX integration

Switch backend from `mock` to `http`:

```toml
[[adapters]]
kind = "offchain"
id = "binance-bridge"
x402_version = 2
scheme = "exact"
networks = ["offchain:binance"]

[adapters.backend]
type = "http"
base_url = "https://payments.internal"
verify_path = "/api/v1/x402/verify"
settle_path = "/api/v1/x402/settle"
api_key_env = "BINANCE_BRIDGE_API_KEY"
timeout_seconds = 8
```

Your external system only needs to implement two endpoints: verify + settle.

## Architecture

See: [docs/x402_v2_chain_agnostic_architecture.md](../docs/x402_v2_chain_agnostic_architecture.md)
