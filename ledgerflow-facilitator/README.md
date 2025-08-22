# ledgerflow-facilitator

Axum-based server that implements an x402-compliant Facilitator for LedgerFlow.

- Endpoints: `/verify`, `/settle`, `/supported`
- Networks: uses `x402-rs` provider cache and USDC EIP-3009 flow (scheme: `exact` on EVM)

## Configure

Set Ethereum JSON-RPC endpoints via env (see x402-rs README for supported keys):

- BASE_MAINNET_RPC_URL
- BASE_SEPOLIA_RPC_URL
- ETHEREUM_MAINNET_RPC_URL
- ...

Optionally set HOST and PORT:

- HOST=0.0.0.0
- PORT=8080

## Run

Use cargo from the workspace root:

```sh
cargo run -p ledgerflow-facilitator
```

## API

- GET /verify → schema info
- POST /verify → VerifyRequest -> VerifyResponse
- GET /settle → schema info
- POST /settle → SettleRequest -> SettleResponse
- GET /supported → supported kinds

For request/response types, see the `x402-rs` crate.
