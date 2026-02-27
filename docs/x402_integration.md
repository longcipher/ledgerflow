# x402 Integration Design (EVM, v2)

This document defines the current EVM integration behavior in LedgerFlow.

For architecture context, see:
- `docs/x402_v2_chain_agnostic_architecture.md`
- `docs/scheme_exact_evm.md`

## Scope

This page is strictly for:
- x402 version 2
- EVM (`network = eip155:<chainId>`)
- `scheme = exact`
- `assetTransferMethod = eip3009`

## Protocol Contract

### Supported

`GET /supported` advertises:
- `kinds[].x402Version = 2`
- CAIP-2 network identifiers (`eip155:*`)
- extension `exact-eip3009`
- signer hints by chain ID

### Verify

`POST /verify` accepts a typed v2 request:
- `x402Version = 2`
- `paymentPayload`
- `paymentRequirements`

Core invariants:
1. `paymentPayload.accepted == paymentRequirements`
2. `scheme == exact`
3. `network.namespace == eip155` and chain ID matches configured chain
4. `assetTransferMethod` is absent or `eip3009`
5. `authorization.to == payTo`
6. `authorization.value == amount`
7. authorization is inside the valid time window
8. signature/nonce structure is valid
9. EVM simulation passes (vault wrapper call)

### Settle

`POST /settle` accepts the same typed v2 body and performs:
- the same semantic checks as verify
- on-chain settlement through:
  - `PaymentVault.depositWithAuthorization(orderId, from, value, validAfter, validBefore, nonce, v, r, s)`

## Canonical EVM Settlement Path

LedgerFlow uses vault-wrapper settlement as canonical path.

Reason:
- preserves event linkage (`DepositReceived(payer, orderId, amount)`)
- keeps indexer behavior stable
- binds settlement to `orderId` carried as EIP-3009 nonce

## Order ID Mapping

For EVM exact settlements in this repo:
- `orderId = authorization.nonce`
- nonce must be 32 bytes

This keeps replay safety aligned with EIP-3009 nonce semantics and preserves deterministic event correlation.

## MCP Integration

`ledgerflow-mcp` exposes:
- `x402_supported`
- `x402_verify`
- `x402_settle`

Current behavior:
- MCP is a v2 proxy that forwards requests to facilitator `/supported`, `/verify`, `/settle`.

## Deprecated v1 Notes

Historical v1 EVM notes are no longer normative for current code paths.

If you are integrating now, use v2 typed payloads only.
