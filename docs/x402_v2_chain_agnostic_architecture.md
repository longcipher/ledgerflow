# LedgerFlow x402 v2 Chain-Agnostic Architecture

This architecture aligns with x402 v2 principles:
- `x402Version = 2`
- CAIP-like `network` identifiers (`namespace:reference`)
- protocol layer decoupled from settlement rails
- dynamic adapter registration for easy integration

## Goals

- Keep the facilitator simple and composable.
- Make CEX and other centralized/offchain payment rails first-class.
- Avoid hard-coding chain-specific logic into HTTP handlers.

## Minimal Architecture

```text
Client / Resource Server
        |
        | POST /verify, /settle (x402 v2 payloads)
        v
+-------------------------------+
| Protocol Layer (Axum)         |
| - Parse x402 request          |
| - Route by version/scheme/net |
+---------------+---------------+
                |
                v
+-------------------------------+
| Adapter Registry              |
| - Match (v, scheme, network)  |
| - Dispatch to selected adapter|
+---------------+---------------+
                |
     +----------+----------+
     |                     |
     v                     v
+----------------+   +----------------------+
| Offchain/CEX   |   | Onchain Adapters     |
| Adapter        |   | (optional, pluggable)|
| - mock/http    |   | eip155/solana/aptos  |
+-------+--------+   +----------------------+
        |
        v
+-------------------------------+
| External payment system       |
| (CEX, internal ledger, PSP)   |
+-------------------------------+
```

## Core Contracts

### Facilitator HTTP API
- `GET /supported`
- `POST /verify`
- `POST /settle`
- `GET /health`

### Adapter match key
- `x402Version`
- `scheme`
- `network` pattern (`offchain:*`, `offchain:binance`, etc.)

### Offchain backend contract
The built-in offchain adapter can call any external system via HTTP:

- Verify endpoint input:
```json
{
  "network": "offchain:binance",
  "paymentPayload": {"...": "..."},
  "paymentRequirements": {"...": "..."}
}
```

- Verify endpoint output:
```json
{
  "valid": true,
  "payer": "cex:user:alice",
  "reason": null
}
```

- Settle endpoint output:
```json
{
  "success": true,
  "payer": "cex:user:alice",
  "transaction": "cex-tx-123",
  "reason": null
}
```

## Why this is easy to integrate

- New payment systems do not require HTTP API changes.
- Integration is config-first (`[[adapters]]`) rather than code-first.
- Any centralized payment system can be connected via two HTTP endpoints.

## Current default in this repo

- Offchain adapter enabled by default (`offchain:*`) with `mock` backend.
- Production mode switches backend to `http` and points to your CEX/PSP bridge.
