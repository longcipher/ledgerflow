# Unichain Sepolia Deployment Guide

This guide explains how to deploy the LedgerFlow Vault contract to Unichain Sepolia testnet.

## Network Configuration

The Unichain Sepolia network has been configured in `foundry.toml` with the following settings:

- **RPC URL**: <https://sepolia.unichain.org>
- **Network Name**: unichain_sepolia
- **Block Explorer**: <https://sepolia.uniscan.xyz/>

## Prerequisites

1. **Install Foundry** (if not already installed):

   ```bash
   curl -L https://foundry.paradigm.xyz | bash
   foundryup
   ```

2. **Set up environment variables**:

   ```bash
   export PRIVATE_KEY=your_private_key_here
   export ETHERSCAN_API_KEY=your_etherscan_api_key_here  # Optional, for contract verification
   ```

3. **Get Unichain Sepolia testnet tokens**:
   - You can get testnet ETH from Unichain Sepolia faucet
   - Bridge testnet tokens from other networks if available

## Deployment Methods

### Method 1: Using the Deployment Script (Recommended)

```bash
# Deploy the contract
./deploy_unichain_sepolia.sh deploy

# Verify the contract (if needed separately)
export CONTRACT_ADDRESS=your_deployed_contract_address
./deploy_unichain_sepolia.sh verify
```

### Method 2: Using Forge Commands Directly

```bash
# Deploy and verify in one command
forge script script/PaymentVault.s.sol:PaymentVaultScript \
    --rpc-url unichain_sepolia \
    --private-key $PRIVATE_KEY \
    --broadcast \
    --verify \
    --etherscan-api-key $ETHERSCAN_API_KEY \
    -vvvv

# Or deploy without verification
forge script script/PaymentVault.s.sol:PaymentVaultScript \
    --rpc-url unichain_sepolia \
    --private-key $PRIVATE_KEY \
    --broadcast \
    -vvvv
```

### Method 3: Verify Existing Contract

```bash
forge verify-contract \
    --rpc-url unichain_sepolia \
    --etherscan-api-key $ETHERSCAN_API_KEY \
    <CONTRACT_ADDRESS> \
    src/PaymentVault.sol:PaymentVault
```

## Important Notes

1. **USDC Address**: The deployment script currently uses a placeholder USDC address. You need to update it with the actual USDC contract address on Unichain Sepolia.

2. **Private Key Security**: Never commit your private key to version control. Use environment variables or consider using hardware wallets for production deployments.

3. **Gas Costs**: Make sure you have enough ETH in your wallet to cover deployment gas costs.

4. **Network Verification**: Always double-check that you're deploying to the correct network before broadcasting transactions.

## Troubleshooting

- **RPC Connection Issues**: Verify that the Unichain Sepolia RPC URL is accessible
- **Insufficient Gas**: Increase your gas limit if deployment fails
- **Verification Failures**: Check if the block explorer API is working and your API key is valid

## Useful Commands

```bash
# Check your wallet balance on Unichain Sepolia
cast balance <YOUR_ADDRESS> --rpc-url unichain_sepolia

# Get the current gas price
cast gas-price --rpc-url unichain_sepolia

# Check if contract is deployed
cast code <CONTRACT_ADDRESS> --rpc-url unichain_sepolia
```
