# x402 Integration Design (EVM first)

This document explains how LedgerFlow integrates the x402 protocol starting on EVM networks, while preserving our minimal on-chain design: the vault only emits an event that binds `orderId` to the deposited `amount`.

## Goals

- Support x402 V1 with the `exact` scheme on EVM via EIP-3009 (USDC).
- Keep on-chain state minimal: still only emit `DepositReceived(payer, orderId, amount)`.
- Make order IDs distributed (no server API required), unique, traceable, and verifiable.
- Remain compatible with agents and MCP flows (e.g., cryo-mcp + payflow).

## Overview

x402 `exact` on EVM settles by invoking `EIP-3009` `transferWithAuthorization` on the token. To retain our on-chain event coupling between `orderId` and `amount`, we introduce a wrapper in `PaymentVault`:

- `depositWithAuthorization(orderId, from, value, validAfter, validBefore, nonce, v, r, s)`
  - Enforces `nonce == orderId` and `to == vault`.
  - Internally calls token `transferWithAuthorization`.
  - Emits `DepositReceived(from, orderId, value)` on success.

This enables a facilitator to submit the same EIP-3009 authorization, but via the vault, so the on-chain `orderId ↔ amount` linkage is preserved without additional storage.

## Order ID (distributed) design

Clients generate the `orderId` locally and set it as the EIP-3009 `nonce`. To ensure global uniqueness and verifiability:

- Define an “Order Manifest” payload (off-chain) with fields:
  - brokerId (string)
  - accountId (int64)
  - payer (address)
  - chainId (uint256)
  - vault (address)
  - asset (address)
  - clientSalt (bytes16 random)
  - issuedAt (uint64, seconds)
  - expiresAt (uint64, optional)
  - amount (string, optional; for analytics/tracing, not enforced on-chain)
- Compute `orderId = keccak256(abi.encodePacked(brokerId, accountId, payer, chainId, vault, asset, clientSalt, issuedAt))`.
  - Including `payer` ensures global uniqueness even though EIP-3009 nonces are per-payer.
  - Include `chainId` and `vault` to prevent cross-network/address collisions.
- Optionally, sign the manifest via EIP-712 with domain `{ name: "LedgerFlow-OrderID", version: "1", chainId, verifyingContract: vault }`, producing `manifestSignature` held by the client.
  - This yields JWT-like verifiability: anyone with manifest + signature can verify the origin offline.
- Set the EIP-3009 `authorization.nonce = orderId` and `authorization.to = vault`.

No server API is needed to mint order ids. Orders created via our Balancer API remain supported; both paths coexist.

## x402 flow mapping

- Resource server returns 402 with PaymentRequirements that include:
  - `scheme: "exact"`, `network`, `payTo = vaultAddress`, `asset = usdcAddress`, `maxTimeoutSeconds`, `maxAmountRequired`.
- Client creates `orderId` as above and signs an EIP-3009 authorization (USDC EIP-3009).
- Client sends the protected request with `X-PAYMENT` header per x402 specs.
- Resource server verifies (locally or POST /verify) and settles via a facilitator (POST /settle) that broadcasts a call to:
  - `PaymentVault.depositWithAuthorization(orderId, from, value, validAfter, validBefore, orderId, v, r, s)`.
- Vault calls token `transferWithAuthorization` and emits our `DepositReceived` event.
- Indexer watches `DepositReceived` and updates DB.

If a third-party facilitator settles by calling the token directly, our event would not be emitted. To preserve our event linkage, use a facilitator that calls the vault wrapper (we will provide one in `ledgerflow-balancer`).

## Verification notes

- Basic checks: network, asset, `authorization.to == vault`, time window, `value <= maxAmountRequired`.
- Simulation: eth_call the token’s `transferWithAuthorization` (or the vault wrapper) to check revert reasons before sending.
- Replay safety is native: EIP-3009 enforces nonce uniqueness per payer. Our `orderId` derivation includes `payer`, ensuring global uniqueness for our DB.

## MCP integration (human-friendly UX)

- Provide a small MCP server for LedgerFlow tools (Rust or TS) that:
  - Can create an x402 header client-side (generate `orderId`, sign EIP-3009) and send the request.
  - Or act as a local “payflow-like” facilitator that posts to our `/settle` and funds gas.
- This mirrors `cryo-mcp` with `payflow`, but targets our vault wrapper to preserve events.

## Database impact

- No schema changes required. Events remain `DepositReceived(payer, orderId, amount)`.
- Indexer stays the same.
- Optional hardening: accept multiple deposits with distinct `orderId` per payer, while preserving uniqueness via our derivation (low collision probability).

## Rollout plan

1) Contracts (EVM): add `depositWithAuthorization` (done) and tests.
2) Balancer: implement `/x402/supported`, `/x402/verify`, `/x402/settle` and config for EVM networks + facilitator key.
3) CLI/SDK: helpers to
   - generate `orderId` (v1 algorithm) and EIP-3009 authorization;
   - call settle endpoint; or submit directly if the user wants to pay gas.
4) Docs and examples: end-to-end example using Base Sepolia USDC.
5) MCP server: optional for agent UX, modeled after cryo-mcp + payflow.

---

For the EVM-only alpha, the canonical settlement path is through `PaymentVault.depositWithAuthorization` to ensure event emission and preserve our elegant on-chain mapping.
