# LedgerFlow ETH CLI

A command-line interface for interacting with the LedgerFlow PaymentVault smart contract on Ethereum using Rust and Alloy.

## Features

- **Standard Deposit**: Deposit USDC with prior approval
- **Permit Deposit**: Gas-efficient deposit using ERC-2612 permit signatures
- **Withdraw**: Owner-only withdrawal of all vault funds
- **Cross-chain Support**: Works with any EVM-compatible blockchain
- **Safe Operations**: Built-in balance and permission checks

## Installation

```bash
cd ledgerflow-eth-cli
cargo build --release
```

## Usage

### Standard Deposit

Requires prior USDC approval to the PaymentVault contract:

```bash
./target/release/ledgerflow-eth-cli deposit \
  --rpc-url "https://sepolia.unichain.org" \
  --private-key "0x..." \
  --contract-address "0x..." \
  --order-id "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef" \
  --amount 1000000
```

### Permit Deposit (Gas Efficient)

Combines approval and deposit in a single transaction:

```bash
./target/release/ledgerflow-eth-cli deposit-with-permit \
  --rpc-url "https://sepolia.unichain.org" \
  --private-key "0x..." \
  --contract-address "0x..." \
  --order-id "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef" \
  --amount 1000000 \
  --deadline 1735689600
```

### Withdraw (Owner Only)

Withdraw all USDC from the vault:

```bash
./target/release/ledgerflow-eth-cli withdraw \
  --rpc-url "https://sepolia.unichain.org" \
  --private-key "0x..." \
  --contract-address "0x..."
```

## Parameters

- `--rpc-url`: The RPC endpoint URL for the blockchain network
- `--private-key`: Your wallet's private key (hex format, with or without 0x prefix)
- `--contract-address`: The deployed PaymentVault contract address
- `--order-id`: 32-byte order identifier (hex format, with or without 0x prefix)
- `--amount`: Amount in USDC base units (e.g., 1000000 = 1 USDC)
- `--deadline`: Unix timestamp for permit expiration

## Order ID Format

Order IDs must be 32 bytes (64 hex characters). You can generate them using:

```bash
# From a string
echo -n "order-123" | sha256sum

# From a UUID
python3 -c "import uuid; print(uuid.uuid4().hex + '0' * 32)"[:64]
```

## Security Notes

1. **Private Keys**: Never commit private keys to version control
2. **Environment Variables**: Use environment variables for sensitive data:

   ```bash
   export PRIVATE_KEY="0x..."
   export RPC_URL="https://..."
   ```

3. **Testnet First**: Always test on testnets before mainnet deployment
4. **Owner Operations**: Only the contract owner can withdraw funds

## Error Handling

The CLI provides detailed error messages for common issues:

- Insufficient USDC balance
- Insufficient allowance (for standard deposits)
- Invalid order ID format
- Contract not found
- Permission denied (for withdrawals)

## Examples

### Environment Variables

```bash
# Set environment variables
export PRIVATE_KEY="0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
export RPC_URL="https://sepolia.unichain.org"
export VAULT_ADDRESS="0x742d35Cc6634C0532925a3b8D11C5d2B7e5B3F6E"

# Use with CLI
./target/release/ledgerflow-eth-cli deposit \
  --rpc-url "$RPC_URL" \
  --private-key "$PRIVATE_KEY" \
  --contract-address "$VAULT_ADDRESS" \
  --order-id "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef" \
  --amount 1000000
```

### Batch Operations

```bash
#!/bin/bash
# deposit_orders.sh - Batch deposit script

ORDERS=(
  "0x1111111111111111111111111111111111111111111111111111111111111111"
  "0x2222222222222222222222222222222222222222222222222222222222222222"
  "0x3333333333333333333333333333333333333333333333333333333333333333"
)

for order in "${ORDERS[@]}"; do
  echo "Depositing for order: $order"
  ./target/release/ledgerflow-eth-cli deposit-with-permit \
    --rpc-url "$RPC_URL" \
    --private-key "$PRIVATE_KEY" \
    --contract-address "$VAULT_ADDRESS" \
    --order-id "$order" \
    --amount 1000000 \
    --deadline $(date -d "+1 hour" +%s)
done
```

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Linting

```bash
cargo clippy
```

### Formatting

```bash
cargo fmt
```

## Dependencies

- [Alloy](https://alloy.rs/) - Ethereum library for Rust
- [Clap](https://clap.rs/) - Command line argument parser
- [Tokio](https://tokio.rs/) - Async runtime

## License

This project is licensed under the Apache License 2.0 - see the main project LICENSE file for details.
