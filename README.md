# LedgerFlow Vault

A secure and efficient vault for receiving USDC payments for orders. It emits an event for each deposit, which an off-chain indexer can use to associate payments with order IDs. The owner can withdraw all funds.

## Features

- **Standard Deposits**: Users can deposit USDC after approving the contract
- **Permit Deposits**: Users can deposit USDC using ERC-2612 permit signatures for gas efficiency
- **Owner Withdrawals**: Contract owner can withdraw all funds
- **Token Recovery**: Emergency function to recover accidentally sent tokens
- **Event Logging**: All deposits and withdrawals emit events for off-chain tracking
- **Cross-Chain Deployment**: Deploy with the same address across multiple EVM-compatible chains

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

## Cross-Chain Deployment

This project supports deploying the same contract address across multiple EVM-compatible chains using CREATE2.

### Quick Cross-Chain Demo

```bash
# Run address prediction demo
./demo_prediction.sh
```

### Multi-Chain Deployment

```bash
# Set your private key
export PRIVATE_KEY=0x...

# Deploy to multiple chains with same addresses
./deploy_multichain.sh
```

### Supported Networks

**Mainnets**: Ethereum, Polygon, Arbitrum, Optimism, Base, Avalanche, BSC
**Testnets**: Sepolia, Polygon Mumbai, Arbitrum Sepolia, Optimism Sepolia, Base Sepolia, Avalanche Fuji, BSC Testnet, Unichain Sepolia

For detailed instructions, see [CROSS_CHAIN_DEPLOYMENT.md](CROSS_CHAIN_DEPLOYMENT.md).

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

### Deployment Examples

#### Standard Deployment

```bash
# Deploy to Unichain Sepolia
export PRIVATE_KEY=your_private_key
export RPC_URL=https://sepolia.unichain.org

forge script script/PaymentVault.s.sol --rpc-url $RPC_URL --private-key $PRIVATE_KEY --broadcast --verify
```

#### UUPS Upgradeable Deployment

```bash
# Deploy upgradeable version with proxy
forge script script/DeployUpgradeable.s.sol --rpc-url $RPC_URL --private-key $PRIVATE_KEY --broadcast --verify

# The script will output:
# - Implementation address (PaymentVault logic contract)
# - Proxy address (The address users interact with)
# - Owner address
# - USDC token address configured
```

#### Upgrading to V2

```bash
# Edit script/UpgradePaymentVault.s.sol and set the correct PROXY_ADDRESS
# Then run the upgrade
forge script script/UpgradePaymentVault.s.sol --rpc-url $RPC_URL --private-key $PRIVATE_KEY --broadcast
```

### Upgrade Commands

Once deployed, you can upgrade the contract to a new implementation:

```bash
# Create a new version of the contract (PaymentVaultV2)
# Then deploy the new implementation
forge script script/UpgradePaymentVault.s.sol --rpc-url <RPC_URL> --private-key <PRIVATE_KEY> --broadcast
```

### Testing Upgrades

Run upgrade-specific tests:

```bash
# Test upgrade functionality
forge test --match-contract PaymentVaultUpgradeTest

# Test with gas reporting
forge test --match-contract PaymentVaultUpgradeTest --gas-report

# Run all tests including upgrades
forge test
```

### Important Notes for UUPS Upgrades

#### Contract Addresses
- **Implementation Address**: The logic contract (changes with each upgrade)
- **Proxy Address**: The permanent address users interact with (never changes)
- **Always use the Proxy Address** for user interactions

#### Upgrade Safety Checklist
1. Ensure only the contract owner can authorize upgrades
2. Test upgrades thoroughly on testnets first
3. Verify that storage layout remains compatible
4. Check that all state variables are preserved
5. Consider upgrade timelock for additional security

#### Storage Layout Compatibility
When creating new versions (V2, V3, etc.):
- ✅ Add new state variables at the end
- ✅ Add new functions
- ❌ Remove existing state variables
- ❌ Change the order of existing state variables
- ❌ Change the type of existing state variables

For detailed upgrade information, see [UUPS_UPGRADE.md](UUPS_UPGRADE.md).

### Available Commands

```bash
# Build contracts
make build

# Run tests
make test

# Deploy standard version to Unichain Sepolia
make deploy-unichain-sepolia

# Deploy upgradeable version with proxy
forge script script/DeployUpgradeable.s.sol --rpc-url https://sepolia.unichain.org --private-key $PRIVATE_KEY --broadcast

# Test upgrade functionality
forge test --match-contract PaymentVaultUpgradeTest

# Verify contract
make verify-unichain-sepolia

# See all available commands
make help
```
