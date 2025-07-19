# LedgerFlow Aptos CLI

A command-line interface for interacting with LedgerFlow payment vault contracts deployed on the Aptos blockchain.

## Features

- **Deposit**: Make USDC deposits to the payment vault with order tracking
- **Withdraw**: Withdraw specific amounts or all funds from the vault (owner only)
- **Info**: Query vault balance, owner, and deposit statistics
- **Account**: Check account balance and information
- **Configuration**: Flexible YAML-based configuration with environment variable support

## Installation

```bash
# Build the CLI tool
cargo build --release

# Or run directly during development
cargo run -- --help
```

## Configuration

Create a configuration file using the init command:

```bash
cargo run -- init --path config.yaml
```

Edit the generated `config.yaml` file with your settings:

```yaml
network:
  node_url: "https://api.devnet.aptoslabs.com/v1"
  chain_id: 4  # 4 for devnet, 2 for testnet, 1 for mainnet
  faucet_url: "https://faucet.devnet.aptoslabs.com"

account:
  # Your private key in hex format
  private_key: "0x1234567890abcdef..."
  address: null  # Optional override

transaction:
  max_gas: 100000
  gas_unit_price: null  # Auto-estimated if null
  expiration_secs: 600
  wait_for_transaction: true

vault:
  # Address where the payment vault contract is deployed
  contract_address: "0xabcdef1234567890..."
  module_name: "payment_vault"
```

### Environment Variables

You can override configuration values using environment variables with the `LEDGERFLOW_` prefix:

```bash
export LEDGERFLOW_ACCOUNT_PRIVATE_KEY="0x1234567890abcdef..."
export LEDGERFLOW_VAULT_CONTRACT_ADDRESS="0xabcdef1234567890..."
export LEDGERFLOW_NETWORK_NODE_URL="https://api.testnet.aptoslabs.com/v1"
```

## Usage

### Initialize Configuration

```bash
# Create a new configuration file
cargo run -- init --path config.yaml

# Force overwrite existing file
cargo run -- init --path config.yaml --force
```

### Deposit USDC

```bash
# Deposit 1 USDC (1000000 micro-USDC assuming 6 decimals)
cargo run -- deposit --order-id "order_12345" --amount 1000000

# Dry run (simulate without submitting)
cargo run -- deposit --order-id "order_12345" --amount 1000000 --dry-run
```

### Withdraw USDC

```bash
# Withdraw specific amount
cargo run -- withdraw --recipient 0xabcdef... --amount 500000

# Withdraw all funds
cargo run -- withdraw --recipient 0xabcdef... --all

# Dry run
cargo run -- withdraw --recipient 0xabcdef... --amount 500000 --dry-run
```

### Query Information

```bash
# Get vault information
cargo run -- info

# Include account balance
cargo run -- info --include-account

# Get account information
cargo run -- account

# Show private key (be careful!)
cargo run -- account --show-private
```

### Output Formats

```bash
# Pretty output (default)
cargo run -- info

# JSON output
cargo run -- info --output json

# Compact output
cargo run -- info --output compact
```

### Verbose Logging

```bash
# Enable debug logging
cargo run -- --verbose info
```

## Command Reference

### Global Options

- `--config <path>`: Specify configuration file path
- `--verbose`: Enable verbose/debug logging
- `--output <format>`: Output format (pretty, json, compact)

### Commands

#### `init`
Initialize a new configuration file.

**Options:**
- `--path <path>`: Configuration file path (default: config.yaml)
- `--force`: Overwrite existing file

#### `deposit`
Deposit USDC to the payment vault.

**Options:**
- `--order-id <id>`: Unique order identifier
- `--amount <amount>`: Amount in USDC smallest units
- `--dry-run`: Simulate without submitting

#### `withdraw`
Withdraw USDC from the payment vault (owner only).

**Options:**
- `--recipient <address>`: Recipient address
- `--amount <amount>`: Amount to withdraw
- `--all`: Withdraw all available funds
- `--dry-run`: Simulate without submitting

#### `info`
Get vault information and statistics.

**Options:**
- `--include-account`: Include account balance information

#### `account`
Get account information and balance.

**Options:**
- `--show-private`: Display private key (use with caution)

## Examples

See `example.sh` for a complete workflow demonstration:

```bash
chmod +x example.sh
./example.sh
```

## Security Considerations

1. **Private Key Security**: Never commit your private key to version control
2. **Configuration Files**: Add `config.yaml` to `.gitignore`
3. **Environment Variables**: Use environment variables in production
4. **Testnet First**: Always test on devnet/testnet before mainnet
5. **Dry Run**: Use `--dry-run` to simulate transactions

## Error Handling

The CLI provides detailed error messages and supports different output formats:

```bash
# JSON error output for programmatic use
cargo run -- --output json deposit --order-id "test" --amount 1000000
```

Common errors:
- **Invalid Configuration**: Check your config file and network settings
- **Insufficient Balance**: Ensure account has enough funds for transactions
- **Invalid Addresses**: Verify vault and recipient addresses
- **Network Issues**: Check node URL and network connectivity

## Development

### Building

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Format code
just format

# Lint code
just lint
```

### Dependencies

The CLI uses the latest Aptos SDK from the devnet branch:

```toml
[dependencies]
aptos-sdk = { git = "https://github.com/aptos-labs/aptos-core", branch = "devnet" }

[patch.crates-io]
merlin = { git = "https://github.com/aptos-labs/merlin" }
x25519-dalek = { git = "https://github.com/aptos-labs/x25519-dalek", branch = "zeroize_v1" }
```

## Integration with LedgerFlow

This CLI tool is part of the larger LedgerFlow ecosystem:

- **Smart Contracts**: `ledgerflow-vault-aptos/` - Move contracts
- **Backend API**: `ledgerflow-balancer/` - Order management
- **Event Indexer**: `ledgerflow-indexer-aptos/` - Event monitoring
- **Database**: `ledgerflow-migrations/` - Schema management

## Troubleshooting

### Common Issues

1. **Transaction Fails**: Check account balance and gas settings
2. **Network Errors**: Verify node URL and network connectivity
3. **Invalid Private Key**: Ensure private key is in correct hex format
4. **Contract Not Found**: Verify vault contract address and deployment

### Debug Mode

Enable verbose logging for detailed debugging:

```bash
RUST_LOG=debug cargo run -- --verbose info
```

### Configuration Validation

Test your configuration:

```bash
# Check account access
cargo run -- account

# Check vault access
cargo run -- info
```

## License

This project is licensed under MIT OR Apache-2.0.
