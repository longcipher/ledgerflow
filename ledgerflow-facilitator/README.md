# LedgerFlow Facilitator (x402 v2)

Chain-agnostic x402 v2 facilitator with adapter registry for pluggable payment backends.

## Highlights

- x402 v2 request/response model via `x402-types` crate
- Chain-agnostic network IDs (CAIP-like `namespace:reference`)
- Adapter registry keyed by `(x402Version, scheme, network pattern)`
- **EVM on-chain adapter** — EIP-3009 `transferWithAuthorization` verify & settle
- **Offchain/CEX adapter** — `mock` and `http` backends  
- Config-first integration — add new payment systems without code changes
- 1 MB request body size limit
- Optional global rate limiting

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/supported` | List supported payment kinds |
| `POST` | `/verify` | Verify a payment payload |
| `POST` | `/settle` | Settle a verified payment |
| `GET` | `/health` | Health check |

## Quick Start

```bash
cd ledgerflow-facilitator
cp config.example.toml config.toml
cargo run
```

Default: `http://0.0.0.0:3402`

## Configuration

Adapters are registered in `config.toml` via `[[adapters]]` sections. Global settings:

```toml
host = "0.0.0.0"
port = 3402
# rate_limit_per_second = 100   # global rate limit (optional)
```

### EVM On-Chain Adapter

Verifies and settles payments using EIP-3009 `transferWithAuthorization` on any EVM chain.

```toml
[[adapters]]
kind = "evm"
id = "base-sepolia"
enabled = true
x402_version = 2
scheme = "exact"
networks = ["eip155:84532"]
rpc_url = "https://sepolia.base.org"
chain_id = 84532
vault_address = "0xYourPaymentVaultAddress"
signer_key_env = "EVM_SIGNER_PRIVATE_KEY"   # optional – needed for settlement
signers = ["0xYourFacilitatorAddress"]
```

**Verify** checks:
1. Authorization timing (validAfter / validBefore)
2. Amount exactly equals `paymentRequirements.amount`
3. Receiver matches `payTo`
4. On-chain token balance sufficient
5. Authorization nonce not already used
6. Settlement call simulation via `eth_call` (`depositWithAuthorization` when `vault_address` is set, otherwise `transferWithAuthorization`)

`paymentRequirements.extra.assetTransferMethod` is standardized to `eip3009` for EVM in this repo.
If omitted, verification still defaults to EIP-3009.

**Settle** sends `PaymentVault.depositWithAuthorization` on-chain (requires `vault_address` + `signer_key_env`).

### EVM Request Examples

Ready-to-run EVM request examples are available under:

- `examples/evm/verify_request.base-sepolia.json`
- `examples/evm/settle_request.base-sepolia.json`
- `examples/evm/verify_request.ethereum-sepolia.json`
- `examples/evm/verify_request.local-anvil.json`

Quick verify call:

```bash
curl -sS http://127.0.0.1:3402/verify \
  -H 'content-type: application/json' \
  --data @examples/evm/verify_request.base-sepolia.json
```

Quick settle call:

```bash
curl -sS http://127.0.0.1:3402/settle \
  -H 'content-type: application/json' \
  --data @examples/evm/settle_request.base-sepolia.json
```

### Offchain Mock Backend (default)

```toml
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

### Offchain HTTP Backend (production)

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

Your external system implements two endpoints (verify + settle) per the contract defined in `x402_v2_chain_agnostic_architecture.md`.

## Security

- **Request body limit**: 1 MB (prevents oversized payload DoS).
- **Rate limiting**: configurable fixed-window counter via `rate_limit_per_second`. For production, combine with a reverse proxy (nginx, Cloudflare) for per-IP rate limiting.
- **CORS**: allows any origin by default; tighten for production.

## Architecture

See: [x402_v2_chain_agnostic_architecture.md](../docs/x402_v2_chain_agnostic_architecture.md)
