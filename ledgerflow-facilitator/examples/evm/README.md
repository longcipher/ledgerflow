# EVM x402 v2 Examples

These examples target the EVM adapter in `ledgerflow-facilitator`.

## Files

- `verify_request.base-sepolia.json`: sample body for `POST /verify`
- `settle_request.base-sepolia.json`: sample body for `POST /settle`
- `verify_request.ethereum-sepolia.json`: sample `POST /verify` for `eip155:11155111`
- `verify_request.local-anvil.json`: sample `POST /verify` for local anvil `eip155:31337`

## Usage

```bash
curl -sS http://127.0.0.1:3402/verify \
  -H 'content-type: application/json' \
  --data @ledgerflow-facilitator/examples/evm/verify_request.base-sepolia.json
```

```bash
curl -sS http://127.0.0.1:3402/settle \
  -H 'content-type: application/json' \
  --data @ledgerflow-facilitator/examples/evm/settle_request.base-sepolia.json
```

The payload values are placeholders for reproducible structure checks. Replace
addresses, signatures, nonces, and timestamps with real values before using on a chain.

`paymentRequirements.extra.assetTransferMethod` is set to `eip3009` to make the
chosen EVM transfer method explicit.
