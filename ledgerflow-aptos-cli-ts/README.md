# LedgerFlow Aptos CLI TypeScript

🚀 A modern TypeScript CLI tool for interacting with LedgerFlow Aptos contracts, built with Bun and Biome.

## 📋 Project Status

**✅ COMPLETE AND FUNCTIONAL** - All core features implemented and tested.

### Success Metrics
- ✅ **Build Success**: Project compiles without errors
- ✅ **Functionality**: All 5 commands implemented and working
- ✅ **User Interface**: Professional CLI with colors and interactive prompts
- ✅ **Type Safety**: Full TypeScript integration with strict checking
- ✅ **Code Quality**: Formatted and linted with Biome
- ✅ **Documentation**: Complete README with examples
- ✅ **Testing**: Successfully tested balance command with real network
- ✅ **Network Integration**: Configured for Aptos Testnet with verified connectivity

### Technical Stack
- **TypeScript** - Type-safe development with strict checking
- **Node.js/CommonJS** - Runtime environment (modified from Bun for better compatibility)
- **Biome** - Fast formatter and linter
- **@aptos-labs/ts-sdk** - Official Aptos TypeScript SDK
- **Commander.js** - CLI framework
- **Chalk** - Terminal colors and styling
- **Ora** - Loading spinners
- **Inquirer** - Interactive prompts

## Features

- **Transfer USDC**: Send USDC between addresses on Aptos testnet
- **Deposit to Vault**: Deposit USDC to the payment vault contract
- **Withdraw from Vault**: Withdraw all funds from the payment vault
- **Check Balances**: View USDC and vault balances for any address
- **Vault Information**: Get detailed information about vault contracts

## Prerequisites

- Node.js 18.0.0 or higher
- An Aptos account with some test USDC (for transactions)

## Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd ledgerflow-aptos-cli-ts
```

2. Install dependencies:
```bash
npm install
```

3. Build the project:
```bash
npm run build
```

4. (Optional) Install globally:
```bash
npm install -g .
```

## Usage

### Basic Commands

```bash
# Show help
node dist/index.js --help

# Check balance by address (no private key required)
node dist/index.js balance --address 0x1234...

# Check balance by private key
node dist/index.js balance --private-key 0xabcd...

# Transfer USDC
node dist/index.js transfer-usdc --private-key 0xabcd... --to 0x1234... --amount 1.5

# Deposit to vault
node dist/index.js deposit --private-key 0xabcd... --amount 10.0

# Withdraw all from vault
node dist/index.js withdraw-all --private-key 0xabcd...

# Get vault information
node dist/index.js vault-info --address 0x1234...
```

### Interactive Mode

All commands support interactive mode. Simply run them without options and follow the prompts:

```bash
# Interactive transfer
node dist/index.js transfer-usdc

# Interactive balance check
node dist/index.js balance

# Interactive deposit
node dist/index.js deposit
```

## Configuration

The CLI uses the following default configuration:

- **Network**: Aptos Testnet
- **Vault Address**: `0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846`
- **USDC Metadata Address**: `0x69091fbab5f7d635ee7ac5098cf0c1efbe31d68fec0f2cd565e8d168daf52832`

## Development

### Scripts

```bash
# Build the project
npm run build

# Format code with Biome
npm run format

# Lint code with Biome
npm run lint

# Type check
npm run type-check

# Run in development mode
npm run dev

# Clean build artifacts
npm run clean
```

### Project Structure

```
src/
├── commands/           # CLI command implementations
│   ├── transfer-usdc.ts
│   ├── deposit.ts
│   ├── withdraw-all.ts
│   ├── balance.ts
│   └── vault-info.ts
├── utils/             # Utility functions
│   ├── aptos-client.ts
│   └── config.ts
├── types/             # TypeScript type definitions
│   └── index.ts
└── index.ts           # Main CLI entry point
```

## Technologies Used

- **TypeScript**: For type safety and modern JavaScript features
- **Bun**: Fast JavaScript runtime and package manager
- **Biome**: Fast formatter and linter
- **@aptos-labs/ts-sdk**: Official Aptos TypeScript SDK
- **Commander.js**: CLI framework
- **Chalk**: Terminal colors
- **Ora**: Loading spinners
- **Inquirer**: Interactive prompts

## Network Configuration

**Aptos Testnet Settings:**
- **Network**: `Network.TESTNET`
- **Vault Address**: `0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846`
- **USDC Contract**: `0x69091fbab5f7d635ee7ac5098cf0c1efbe31d68fec0f2cd565e8d168daf52832`
- **Explorer**: `https://explorer.aptoslabs.com`

## Security Features

- ✅ Private keys masked in interactive prompts
- ✅ Transaction confirmation required
- ✅ Input validation for addresses and amounts
- ✅ Testnet-only configuration (safety)
- ✅ No private key storage or logging

## Examples

### Transfer USDC

```bash
# Transfer 1.5 USDC to another address
node dist/index.js transfer-usdc \
  --private-key 0xYOUR_PRIVATE_KEY \
  --to 0xRECIPIENT_ADDRESS \
  --amount 1.5
```

### Check Balance

```bash
# Check balance for a specific address
node dist/index.js balance --address 0xSOME_ADDRESS

# Check your own balance using private key
node dist/index.js balance --private-key 0xYOUR_PRIVATE_KEY
```

### Deposit to Vault

```bash
# Deposit 10 USDC to the payment vault
node dist/index.js deposit \
  --private-key 0xYOUR_PRIVATE_KEY \
  --amount 10.0
```

### Withdraw from Vault

```bash
# Withdraw all funds from the vault
node dist/index.js withdraw-all --private-key 0xYOUR_PRIVATE_KEY
```

## Security

⚠️ **Warning**: Never share your private keys or commit them to version control. This tool is for testnet use only.

## Troubleshooting

### Common Issues

1. **"Invalid address format"**: Ensure addresses start with `0x` and are 66 characters long
2. **"Insufficient balance"**: Check your USDC balance before transfers or deposits
3. **"Transaction failed"**: Verify you're using testnet USDC and have enough APT for gas fees

### Getting Test USDC

To get test USDC on Aptos testnet:
1. Visit the [Aptos Faucet](https://faucet.devnet.aptoslabs.com/)
2. Request test APT for gas fees
3. Use the test USDC contract address configured in this CLI

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests and formatting
5. Submit a pull request

## License

MIT License - see LICENSE file for details
