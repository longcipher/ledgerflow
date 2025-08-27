# LedgerFlow Sui CLI

Command-line interface for interacting with LedgerFlow Sui payment vault contracts.

## Overview

The LedgerFlow Sui CLI provides a simple command-line interface to interact with payment vault contracts deployed on the Sui blockchain. It supports deposit, withdrawal, and account management operations with modern TOML configuration.

## Features

- **Deposit**: Make USDC deposits to payment vaults with order tracking
- **Withdraw**: Owner-only withdrawals with recipient specification
- **Account Management**: View account balances and information
- **Vault Information**: Query vault state and balances
- **Multiple Output Formats**: Pretty, JSON, and compact output
- **Dry Run Mode**: Test transactions without executing them
- **TOML Configuration**: Modern configuration format with YAML fallback support
- **Environment Variable Support**: Override config via environment variables

## Installation

### Prerequisites

- Rust 1.70+ with Cargo
- Access to a Sui network (devnet, testnet, or mainnet)

### Build from Source

```bash
# From the project root
cargo build --release -p ledgerflow-sui-cli

# Or from the ledgerflow-sui-cli directory
cd ledgerflow-sui-cli
cargo build --release
```

## Configuration

### Initial Setup

1. Create a configuration file (TOML format recommended):
```bash
ledgerflow-sui-cli init --path config.toml
```

Or create a YAML configuration (legacy format):
```bash
ledgerflow-sui-cli init --path config.yaml
```

Create a configuration file (TOML format recommended):

```bash
ledgerflow-sui-cli init --path config.toml
```

Or create a YAML configuration (legacy format):

```bash
ledgerflow-sui-cli init --path config.yaml
```

Edit the configuration file with your settings:

**TOML Configuration (`config.toml`):**

```toml
# Network configuration
[network]
rpc_url = "https://fullnode.devnet.sui.io:443"
network = "devnet"

[account]
private_key = "0x1234...your_private_key"
key_scheme = "ed25519"

[vault]
package_id = "0xabc123...your_package_id"
vault_object_id = "0xdef456...your_vault_object_id"
usdc_type = "0x2::sui::SUI"  # or your USDC coin type
```

### Configuration Options

#### Network Settings

- `rpc_url`: Sui Full node RPC endpoint
- `ws_url`: WebSocket URL for event subscription (optional)
- `network`: Network name (devnet, testnet, mainnet, localnet)

#### Account Settings

- `private_key`: Your account's private key (hex format)
- `address`: Account address (auto-derived if not specified)
- `key_scheme`: Cryptographic key scheme (ed25519, secp256k1, secp256r1)

#### Transaction Settings

- `gas_budget`: Maximum gas to spend per transaction (in MIST)
- `gas_price`: Gas price override (auto-estimated if not set)
- `expiration_secs`: Transaction expiration timeout
- `wait_for_transaction`: Whether to wait for transaction confirmation

#### Vault Settings

- `package_id`: Address of the deployed payment vault package
- `module_name`: Module name (typically "payment_vault")
- `vault_object_id`: Object ID of the vault shared object
- `usdc_type`: Type identifier for USDC coins

### Environment Variables

You can override configuration using environment variables with the prefix `LEDGERFLOW_SUI_CLI__`:

```bash
# Override network RPC URL
export LEDGERFLOW_SUI_CLI__NETWORK__RPC_URL="https://custom-sui-node.example.com"

# Override gas budget
export LEDGERFLOW_SUI_CLI__TRANSACTION__GAS_BUDGET=20000000

# Set private key securely (recommended)
export SUI_PRIVATE_KEY="your_private_key_here"
```

## Usage

### Basic Commands

#### Initialize Configuration

```bash
ledgerflow-sui-cli init --path config.toml
```

#### Make a Deposit

```bash
# Deposit 100 USDC units with order ID
ledgerflow-sui-cli deposit --order-id "order_12345" --amount 100000000

# Dry run (simulate without executing)
ledgerflow-sui-cli deposit --order-id "order_12345" --amount 100000000 --dry-run
```

#### Withdraw Funds (Owner Only)

```bash
# Withdraw specific amount
ledgerflow-sui-cli withdraw --recipient 0x123abc... --amount 50000000

# Withdraw all funds
ledgerflow-sui-cli withdraw --recipient 0x123abc... --all

# Dry run withdrawal
ledgerflow-sui-cli withdraw --recipient 0x123abc... --amount 50000000 --dry-run
```

#### Get Vault Information

```bash
# Basic vault info
ledgerflow-sui-cli info

# Include account information
ledgerflow-sui-cli info --include-account
```

#### Get Account Information

```bash
# Account balances and address
ledgerflow-sui-cli account

# Show private key (use with caution)
ledgerflow-sui-cli account --show-private
```

### Advanced Options

#### Output Formats

```bash
# Pretty formatted output (default)
ledgerflow-sui-cli info --output pretty

# JSON output for scripting
ledgerflow-sui-cli info --output json

# Compact text output
ledgerflow-sui-cli info --output compact
```

#### Verbose Logging
```bash
# Enable debug logging
ledgerflow-sui-cli --verbose deposit --order-id "order_123" --amount 1000000
```

#### Custom Configuration File
```bash
# Use a specific configuration file
ledgerflow-sui-cli --config ./my-config.yaml account
```

### Environment Variables

Configuration can be overridden using environment variables with the `LEDGERFLOW_` prefix:

```bash
export LEDGERFLOW_NETWORK_RPC_URL="https://fullnode.testnet.sui.io:443"
export LEDGERFLOW_ACCOUNT_PRIVATE_KEY="0x1234..."
export LEDGERFLOW_VAULT_PACKAGE_ID="0xabc123..."

ledgerflow-sui-cli info
```

## Security Considerations

### Private Key Management
- **Never commit private keys to version control**
- Use environment variables for sensitive configuration
- Consider hardware wallets for production use
- Regularly rotate keys and update configurations

### Network Security
- Verify RPC endpoint authenticity
- Use HTTPS/WSS endpoints only
- Monitor for suspicious transaction activity

### Transaction Safety
- Always test with small amounts first
- Use dry-run mode to verify transactions
- Double-check recipient addresses
- Monitor gas costs and set appropriate budgets

## Troubleshooting

### Common Issues

#### "Failed to build Sui client"
- Verify RPC URL is correct and accessible
- Check network connectivity
- Ensure the Sui node is operational

#### "No gas coins available"
- Fund your account with SUI for gas fees
- Use Sui faucet for devnet/testnet: https://discord.gg/sui

#### "Failed to parse package/object ID"
- Ensure IDs are in correct hex format with 0x prefix
- Verify the package is deployed on the target network
- Check that object IDs exist and are accessible

#### "Invalid private key"
- Verify private key format (hex with optional 0x prefix)
- Ensure key matches the specified key scheme
- Check for typos or missing characters

### Debug Mode

Enable verbose logging for detailed error information:
```bash
ledgerflow-sui-cli --verbose <command>
```

## Integration Examples

### Shell Scripting
```bash
#!/bin/bash
# Automated deposit script

ORDER_ID="order_$(date +%s)"
AMOUNT=1000000  # 1 USDC (6 decimals)

# Make deposit and capture result
RESULT=$(ledgerflow-sui-cli deposit --order-id "$ORDER_ID" --amount "$AMOUNT" --output json)

if echo "$RESULT" | jq -r '.status' | grep -q "success"; then
    echo "Deposit successful: $ORDER_ID"
    TX_HASH=$(echo "$RESULT" | jq -r '.transaction_hash')
    echo "Transaction hash: $TX_HASH"
else
    echo "Deposit failed"
    exit 1
fi
```

### JSON Processing
```bash
# Get vault balance as JSON
VAULT_INFO=$(ledgerflow-sui-cli info --output json)
BALANCE=$(echo "$VAULT_INFO" | jq -r '.vault.balance')
echo "Current vault balance: $BALANCE"
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

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Ensure all tests pass
6. Submit a pull request

## License

This project is licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT License ([LICENSE-MIT](../LICENSE-MIT))

at your option.

## Support

For support and questions:
- GitHub Issues: https://github.com/longcipher/ledgerflow/issues
- Documentation: https://github.com/longcipher/ledgerflow/docs
