# LedgerFlow × SpoonOS Integration — Grant (Concise)

- Problem Description
  - Who: long‑tail SaaS/indie/creators; Web3 teams selling subs/API/digital goods; small teams without 24/7 ops.
  - Pain (High): onboarding friction; freeze risk with custodial gateways; cross‑border fees/coverage; multi‑chain reconciliation burden; heavy integration effort.
  - Why now: stablecoins + L2s are production‑ready; agentic ops with SpoonOS can automate guidance and recovery.
  - Impact: accept global payments in minutes; lower ops load; higher conversion; merchants keep custody.

- Business Opportunity
  - Market: fast‑growing stablecoin settlement; underserved “agentic ops” layer for payments.
  - Positioning: LedgerFlow = non‑custodial multi‑chain gateway; SpoonOS = agentic ops/assistant.
  - Model: 0.10–0.25% service fee; SaaS tiers for agent features; enterprise deployments/services.
  - Moat: non‑custodial vaults; audited multi‑chain indexers; open MCP tool ecosystem (no lock‑in).
  - KPIs: conversion↑; support tickets↓; TTI < 1 day; MTTR↓; agent auto‑resolution rate↑.

- Technical Plan
  - MCP tools: create_order, get_order_status, list_pending_orders, investigate_deposit, refund_or_withdraw (confirm‑gated).
  - Agents: Merchant Copilot (Telegram: pay links/QR, error guidance); Ops Supervisor (SLA monitors, reconciliation); DevRel Autodemo.
  - Backend: add read‑only, paginated endpoints + HMAC webhook; scoped tokens via torii‑rs; preserve order_id keccak256(big‑endian) + DB invariants.
  - Security/Reliability: least‑privilege; no key custody; strict JSON schemas; tracing/metrics; fallback LLMs + caching.
  - Milestones: W0 interfaces; W1‑2 MCP + basic agent; W3‑4 ops workflows + webhooks; W5‑6 hardening + E2E on Unichain Sepolia/Aptos.

```text
                              +----------------------+
                              |   Spoon Agents (S)   |
                              | - Merchant Copilot   |
                              | - Ops Supervisor     |
                              +----------+-----------+
                                         | assist/status
                                         v
  +----------------------+     chat       ^      +----------------------+
  |     Payer/Client     |<---------------+----->|     Telegram Bot     |
  +----------------------+                     +------------------------+
                                                    | notify
                                                    v
                                            +------------------+
                                            |     Merchant     |
                                            +------------------+


               +------------------------+  read/write  +------------------+
               |   Balancer API (Axum)  |<------------>|   PostgreSQL DB  |
               |   /orders  /x402/...   |              +------------------+
               +-----------+------------+                     ^
                           | settle (x402 exact)              | update status
                           v                                  |
               +------------------------+           +----------------------+
               |   PaymentVault (EVM)   |---------->|       Indexers       |
               |   deposit/depositWA    |  event:   +----------------------+
               +------------------------+  DepositReceived
                           ^
                           |
                     +-----+------+
                     |    USDC    |  EIP-3009 transferWithAuthorization
                     +------------+
```

x402 flow (exact, EVM):

1. x402 Resource Server -> (402 PaymentRequirements) -> Payer/Client
2. Payer creates EIP-3009 auth (nonce = orderId), sends to Balancer (/x402/settle)
3. Balancer calls PaymentVault.depositWithAuthorization (settlement)
4. Vault emits DepositReceived(payer, orderId, amount)
5. Indexers update DB; Agents query status and notify Merchant
