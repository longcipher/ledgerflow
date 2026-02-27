# Scheme: `exact` on `EVM` (x402 v2)

## Summary

LedgerFlow EVM payments use x402 v2 + EIP-3009 signatures.  
For settlement, the canonical path is `PaymentVault.depositWithAuthorization(...)` so the vault emits `DepositReceived(payer, orderId, amount)` and preserves `orderId -> amount` linkage.

Current transfer method contract in this repo:

- `scheme = "exact"`
- `network = "eip155:<chainId>"`
- `paymentRequirements.extra.assetTransferMethod = "eip3009"`
- facilitator `/supported.extensions` contains `"exact-eip3009"`

## v2 Payment Payload (EVM exact)

`paymentPayload.payload` fields:

- `signature`: hex ECDSA signature (65 bytes, `r||s||v`)
- `authorization`: EIP-3009 transfer authorization
  - `from`
  - `to`
  - `value`
  - `validAfter`
  - `validBefore`
  - `nonce` (32-byte hex, used as `orderId`)

Example payload:

```json
{
  "signature": "0x2d6a7588d6acca505cbf0d9a4a227e0c52c6c34008c8e8986a1283259764173608a2ce6496642e377d6da8dbbf5836e9bd15092f9ecab05ded3d6293af148b571c",
  "authorization": {
    "from": "0x857b06519E91e3A54538791bDbb0E22373e36b66",
    "to": "0x209693Bc6afc0C5328bA36FaF03C514EF312287C",
    "value": "10000",
    "validAfter": "1740672089",
    "validBefore": "1740672154",
    "nonce": "0xf3746613c2d920b5fdabc0856f2aeb2d4f88ee6037b8cc5d04a71a4462f13480"
  }
}
```

## Full x402 v2 Verify/Settle body example

```json
{
  "x402Version": 2,
  "paymentPayload": {
    "x402Version": 2,
    "accepted": {
      "scheme": "exact",
      "network": "eip155:84532",
      "amount": "10000",
      "payTo": "0x209693Bc6afc0C5328bA36FaF03C514EF312287C",
      "maxTimeoutSeconds": 300,
      "asset": "0x0000000000000000000000000000000000000010",
      "extra": {
        "assetTransferMethod": "eip3009"
      }
    },
    "payload": {
      "signature": "0x2d6a7588d6acca505cbf0d9a4a227e0c52c6c34008c8e8986a1283259764173608a2ce6496642e377d6da8dbbf5836e9bd15092f9ecab05ded3d6293af148b571c",
      "authorization": {
        "from": "0x857b06519E91e3A54538791bDbb0E22373e36b66",
        "to": "0x209693Bc6afc0C5328bA36FaF03C514EF312287C",
        "value": "10000",
        "validAfter": "1740672089",
        "validBefore": "1740672154",
        "nonce": "0xf3746613c2d920b5fdabc0856f2aeb2d4f88ee6037b8cc5d04a71a4462f13480"
      }
    },
    "resource": {
      "url": "https://example.com/resource",
      "description": "exact payment example",
      "mimeType": "application/json"
    }
  },
  "paymentRequirements": {
    "scheme": "exact",
    "network": "eip155:84532",
    "amount": "10000",
    "payTo": "0x209693Bc6afc0C5328bA36FaF03C514EF312287C",
    "maxTimeoutSeconds": 300,
    "asset": "0x0000000000000000000000000000000000000010",
    "extra": {
      "assetTransferMethod": "eip3009"
    }
  }
}
```

## Verification checklist

For `exact` on EVM, verification should reject if any check fails:

1. `x402Version == 2`
2. `paymentPayload.accepted == paymentRequirements`
3. `scheme == "exact"`
4. `network.namespace == "eip155"` and chain id matches configured chain
5. `assetTransferMethod` is absent or `"eip3009"`
6. `authorization.to == payTo`
7. `authorization.value == amount`
8. `validAfter <= now` and `validBefore >= now + buffer`
9. `nonce` is 32 bytes
10. `signature` format is valid
11. optional on-chain checks: balance, nonce-used state, simulation

## Settlement path

Settlement submits:

`PaymentVault.depositWithAuthorization(orderId, from, value, validAfter, validBefore, nonce, v, r, s)`

with:

- `orderId = nonce`
- `nonce = authorization.nonce`
- `to` bound to vault/payee checks in verification + vault wrapper

This path preserves the canonical vault event for indexers.
