# Web3 Payment Vault

A simple vault for receiving USDC payments for orders. It emits an event for each deposit, which an off-chain indexer can use to associate payments with order IDs. The owner can withdraw all funds.

## Features

- **Standard Deposits**: Users can deposit USDC after approving the contract
- **Permit Deposits**: Users can deposit USDC using ERC-2612 permit signatures for gas efficiency
- **Owner Withdrawals**: Contract owner can withdraw all funds
- **Token Recovery**: Emergency function to recover accidentally sent tokens
- **Event Logging**: All deposits and withdrawals emit events for off-chain tracking

## Smart Contract Functions

### deposit(bytes32 orderId, uint256 amount)

Standard deposit function requiring prior approval of USDC tokens.

### depositWithPermit(bytes32 orderId, uint256 amount, uint256 deadline, uint8 v, bytes32 r, bytes32 s)

**NEW**: Gas-efficient deposit using ERC-2612 permit signatures. Combines approval and deposit in a single transaction, saving approximately 24% in gas costs.

### withdraw()

Owner-only function to withdraw all USDC from the vault.

### recoverToken(address token, address recipient)

Emergency function to recover any ERC20 tokens accidentally sent to the vault.

## Gas Efficiency Comparison

| Method | Gas Cost | Transactions |
|--------|----------|-------------|
| Traditional (approve + deposit) | ~101,000 gas | 2 |
| **Permit deposit** | ~77,000 gas | 1 |
| **Gas Savings** | **~24,000 gas (24%)** | **1 less transaction** |

## Usage

For detailed usage instructions and code examples, see [PERMIT_USAGE.md](docs/PERMIT_USAGE.md).

## Testing

Run the test suite:

```bash
forge test
```

## Deployment

### Quick Start with Unichain Sepolia

Deploy to Unichain Sepolia testnet using the provided scripts:

```bash
# Set up environment variables
export PRIVATE_KEY=your_private_key_here
export ETHERSCAN_API_KEY=your_etherscan_api_key_here

# Deploy using the convenience script
./deploy_unichain_sepolia.sh deploy

# Or deploy using Make
make deploy-unichain-sepolia
```

For detailed deployment instructions, see [UNICHAIN_DEPLOYMENT.md](UNICHAIN_DEPLOYMENT.md).

### Manual Deployment

Deploy to any network:

```bash
forge script script/PaymentVault.s.sol --rpc-url <RPC_URL> --private-key <PRIVATE_KEY> --broadcast
```

### Available Commands

```bash
# Build contracts
make build

# Run tests
make test

# Deploy to Unichain Sepolia
make deploy-unichain-sepolia

# Verify contract
make verify-unichain-sepolia

# See all available commands
make help
```
