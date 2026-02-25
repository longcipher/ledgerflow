<div align="center">
  <img src="/assets/ledgerflow_banner.png" alt="LedgerFlow Banner" />
</div>

# LedgerFlow

[![DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/longcipher/ledgerflow)
[![Context7](https://img.shields.io/badge/Website-context7.com-blue)](https://context7.com/longcipher/ledgerflow)

![hpx](https://socialify.git.ci/longcipher/ledgerflow/image?font=Source+Code+Pro&language=1&name=1&owner=1&pattern=Circuit+Board&theme=Auto)

## MVP

[![LedgerFlow MVP](https://img.youtube.com/vi/YedfwqCenMQ/maxresdefault.jpg)](https://www.youtube.com/watch?v=YedfwqCenMQ)

**A chain-agnostic x402 v2 facilitator for AI-native payments.**

## Executive Summary

LedgerFlow is open infrastructure that helps resource servers adopt **x402 v2** once and settle payments across **offchain and onchain rails** without changing their protocol-facing API.

### Problem

x402 adoption is currently constrained by settlement fragmentation:

- integrations are often chain-specific
- offchain liquidity rails (CEX/PSP/internal ledgers) are poorly standardized
- teams must rewrite logic per payment backend

### Our Solution

LedgerFlow introduces a minimal facilitator architecture:

- protocol layer: x402 v2 verification/settlement interface
- dispatch layer: adapter registry keyed by `(x402Version, scheme, network pattern)`
- settlement layer: pluggable adapters (`offchain:*` today, onchain adapters next)

### Why This Is Grant-Worthy

- unlocks practical x402 usage now (offchain rails are first-class)
- reduces integration cost to config + two bridge endpoints
- creates reusable public goods for AI agents and API marketplaces

## Technical Innovation

1. **Chain-agnostic dispatch key**
   A strict routing contract over `x402Version + scheme + network` decouples protocol handling from settlement implementation.
2. **First-class offchain namespace**
   Supports `offchain:*` and `offchain:<provider>` networks so CEX/PSP integrations are native to the flow.
3. **Config-first expansion**
   New systems are integrated via `[[adapters]]` config, not HTTP/API rewrites.
4. **Stable facilitator surface**
   Single interface for operators: `POST /verify`, `POST /settle`, `GET /supported`, `GET /health`.

## Latest Architecture (Only)

```text
Client / Resource Server
        |
        | x402 v2 (verify/settle)
        v
+-------------------------------+
| Protocol Layer (Axum)         |
| - Parse x402 v2 requests      |
| - Normalize/validate payloads |
+---------------+---------------+
                |
                v
+-------------------------------+
| Adapter Registry              |
| - Match by (v, scheme, net)   |
| - Dispatch selected adapter    |
+---------------+---------------+
                |
      +---------+----------+
      |                    |
      v                    v
+-------------+    +------------------+
| Offchain    |    | Onchain Adapters |
| CEX/PSP     |    | (pluggable)      |
| mock/http   |    | roadmap          |
+------+------+    +------------------+
       |
       v
+-------------------------------+
| External Payment System       |
| CEX / PSP / Internal Ledger   |
+-------------------------------+
```

Detailed design: [docs/x402_v2_chain_agnostic_architecture.md](./docs/x402_v2_chain_agnostic_architecture.md)

## Current Implementation Status

Implemented now:

- x402 v2 facilitator core
- adapter registry and dispatch
- offchain adapter with:
  - `mock` backend (development)
  - `http` backend (production bridge)
- integration tests for v2 offchain verify/settle flow

## Integration Contract (CEX/PSP Bridge)

Your bridge only needs two endpoints.

Input payload:

```json
{
  "network": "offchain:binance",
  "paymentPayload": {"...": "..."},
  "paymentRequirements": {"...": "..."}
}
```

Verify output:

```json
{
  "valid": true,
  "payer": "cex:user:alice",
  "reason": null
}
```

Settle output:

```json
{
  "success": true,
  "payer": "cex:user:alice",
  "transaction": "cex-tx-123",
  "reason": null
}
```

## Grant Program Deliverables

### Milestone 1: Production Connector Pack

- hardened bridge templates for major offchain payment providers
- reference auth/retry/idempotency patterns
- operator runbooks

### Milestone 2: Onchain Adapter Expansion

- at least one audited onchain adapter under the same registry model
- conformance tests aligned with x402 v2 semantics

### Milestone 3: Reliability and Security Hardening

- structured observability for verify/settle lifecycle
- replay-safety and failure-mode test matrix
- deployment profiles for resource-server operators

## Success Metrics (Program-Friendly)

- integration time from "days of custom code" to "hours of config + bridge"
- number of production connectors shipped
- number of external services adopting `/verify` + `/settle`
- end-to-end settlement success rate and mean settlement latency

## Quick Start

```bash
cp ledgerflow-facilitator/config.example.toml ledgerflow-facilitator/config.toml
cargo run -p ledgerflow-facilitator -- --config ledgerflow-facilitator/config.toml
cargo test -p ledgerflow-facilitator --test v2_offchain_adapter_tests
```

Default bind: `0.0.0.0:3402`

## References

- x402 official site: [https://www.x402.org/](https://www.x402.org/)
- x402 docs: [https://docs.x402.org/introduction](https://docs.x402.org/introduction)
- x402 whitepaper: [https://www.x402.org/x402-whitepaper.pdf](https://www.x402.org/x402-whitepaper.pdf)
- x402-rs: [https://github.com/x402-rs/x402-rs](https://github.com/x402-rs/x402-rs)

## SDK Ecosystem

LedgerFlow provides multi-language SDKs for integrating with the Balancer API:

| Package | Language | Description |
|---------|----------|-------------|
| [ledgerflow-sdk-rs](./ledgerflow-sdk-rs/) | Rust | Core SDK — typed client, models, `generate_order_id` |
| [ledgerflow-sdk-py](./ledgerflow-sdk-py/) | Python | PyO3 bindings — `pip install` via maturin |
| [ledgerflow-sdk-ts](./ledgerflow-sdk-ts/) | TypeScript/Node.js | napi-rs bindings — native `.node` addon |
| [ledgerflow-sdk-frontend](./ledgerflow-sdk-frontend/) | Browser/WASM | wasm-bindgen bindings — ESM import |

