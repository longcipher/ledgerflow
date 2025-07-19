#!/bin/bash

# Script to deposit 1 USDC to the LedgerFlow Payment Vault using Aptos CLI
# This script interacts with the deployed vault on Aptos testnet

set -e

# Configuration
VAULT_ADDRESS="0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846"
USDC_METADATA_ADDRESS="0x69091fbab5f7d635ee7ac5098cf0c1efbe31d68fec0f2cd565e8d168daf52832"
NETWORK="testnet"
AMOUNT="1000000"  # 1 USDC (6 decimals: 1 * 10^6)

# Generate a unique order ID (32 bytes hex string)
ORDER_ID="0x$(openssl rand -hex 32)"

echo "=== LedgerFlow Payment Vault - Deposit USDC ==="
echo "Vault Address: $VAULT_ADDRESS"
echo "Amount: $AMOUNT micro-USDC (1 USDC)"
echo "Order ID: $ORDER_ID"
echo "Network: $NETWORK"
echo ""

# Check if profile exists
echo "Checking Aptos CLI profile..."
if ! aptos config show-profiles 2>/dev/null | grep -q "default"; then
    echo "❌ No Aptos CLI profile found. Please run 'aptos init' first."
    exit 1
fi

# Show current account
ACCOUNT=$(aptos config show-profiles | grep '"account":' | head -1 | awk -F'"' '{print $4}')
echo "Using account: $ACCOUNT"
echo ""

# Check USDC balance first
echo "Checking USDC balance..."
BALANCE_RESULT=$(aptos account list --query resources --account $ACCOUNT --url https://api.testnet.aptoslabs.com/v1 2>/dev/null || echo "")

if [[ $BALANCE_RESULT == *"$USDC_METADATA_ADDRESS"* ]]; then
    echo "✅ USDC balance found"
else
    echo "⚠️  Warning: No USDC balance found. You may need to acquire testnet USDC first."
    echo "   You can get testnet USDC from: https://faucet.circle.com/"
fi
echo ""

# Execute the deposit transaction
echo "Executing deposit transaction..."
aptos move run \
    --function-id ${VAULT_ADDRESS}::payment_vault_fa::deposit \
    --args address:$VAULT_ADDRESS \
    --args hex:$ORDER_ID \
    --args u64:$AMOUNT \
    --url https://api.testnet.aptoslabs.com/v1 \
    --assume-yes

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Deposit transaction completed successfully!"
    echo "Order ID: $ORDER_ID"
    echo "Amount: $AMOUNT micro-USDC"
    echo ""
    echo "🔍 You can view the transaction on Aptos Explorer:"
    echo "   https://explorer.aptoslabs.com/account/$VAULT_ADDRESS?network=testnet"
else
    echo ""
    echo "❌ Deposit transaction failed!"
    exit 1
fi
