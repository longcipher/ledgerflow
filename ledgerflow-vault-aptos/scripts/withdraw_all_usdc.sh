#!/bin/bash

# Script to withdraw all USDC from the LedgerFlow Payment Vault using Aptos CLI
# This script can only be executed by the vault owner

set -e

# Configuration
VAULT_ADDRESS="0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846"
NETWORK="testnet"

echo "=== LedgerFlow Payment Vault - Withdraw All USDC ==="
echo "Vault Address: $VAULT_ADDRESS"
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

# Check if current account is the vault owner
echo "Checking vault ownership..."
OWNER_RESULT=$(aptos move view \
    --function-id ${VAULT_ADDRESS}::payment_vault::get_owner \
    --args address:$VAULT_ADDRESS \
    --url https://api.testnet.aptoslabs.com/v1 2>/dev/null || echo "ERROR")

if [[ $OWNER_RESULT == *"ERROR"* ]]; then
    echo "❌ Failed to check vault ownership. Is the vault initialized?"
    exit 1
fi

VAULT_OWNER=$(echo $OWNER_RESULT | grep -o '0x[a-fA-F0-9]*' | head -1)
echo "Vault Owner: $VAULT_OWNER"

# Normalize addresses for comparison by removing 0x prefix and leading zeros
ACCOUNT_NORMALIZED=$(echo "$ACCOUNT" | sed 's/^0x//' | sed 's/^0*//g')
OWNER_NORMALIZED=$(echo "$VAULT_OWNER" | sed 's/^0x//' | sed 's/^0*//g')

if [[ "$ACCOUNT_NORMALIZED" != "$OWNER_NORMALIZED" ]]; then
    echo "❌ Error: Current account ($ACCOUNT) is not the vault owner ($VAULT_OWNER)"
    echo "   Only the vault owner can withdraw funds."
    exit 1
fi

echo "✅ Ownership verified"
echo ""

# Check vault balance
echo "Checking vault balance..."
BALANCE_RESULT=$(aptos move view \
    --function-id ${VAULT_ADDRESS}::payment_vault::get_balance \
    --args address:$VAULT_ADDRESS \
    --url https://api.testnet.aptoslabs.com/v1 2>/dev/null || echo "ERROR")

if [[ $BALANCE_RESULT == *"ERROR"* ]]; then
    echo "❌ Failed to check vault balance"
    exit 1
fi

VAULT_BALANCE=$(echo $BALANCE_RESULT | grep -o '[0-9]*' | head -1)
echo "Current vault balance: $VAULT_BALANCE micro-USDC"

if [[ "$VAULT_BALANCE" == "0" ]]; then
    echo "⚠️  Vault is empty. Nothing to withdraw."
    exit 0
fi

echo ""

# Prompt for recipient address
read -p "Enter recipient address (press Enter to use current account): " RECIPIENT
if [[ -z "$RECIPIENT" ]]; then
    RECIPIENT=$ACCOUNT
fi

echo "Recipient: $RECIPIENT"
echo ""

# Confirm withdrawal
echo "⚠️  You are about to withdraw ALL funds ($VAULT_BALANCE micro-USDC) from the vault."
read -p "Do you want to continue? (y/N): " CONFIRM

if [[ "$CONFIRM" != "y" && "$CONFIRM" != "Y" ]]; then
    echo "Withdrawal cancelled."
    exit 0
fi

echo ""

# Execute the withdrawal transaction
echo "Executing withdraw_all transaction..."
aptos move run \
    --function-id ${VAULT_ADDRESS}::payment_vault_fa::withdraw_all \
    --args address:$VAULT_ADDRESS \
    --args address:$RECIPIENT \
    --url https://api.testnet.aptoslabs.com/v1 \
    --assume-yes

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Withdrawal transaction completed successfully!"
    echo "Amount withdrawn: $VAULT_BALANCE micro-USDC"
    echo "Recipient: $RECIPIENT"
    echo ""
    echo "🔍 You can view the transaction on Aptos Explorer:"
    echo "   https://explorer.aptoslabs.com/account/$VAULT_ADDRESS?network=testnet"
else
    echo ""
    echo "❌ Withdrawal transaction failed!"
    exit 1
fi
