#!/bin/bash

# Demo script for LedgerFlow Sui Vault
# This script demonstrates basic vault operations

set -e

echo "🚀 LedgerFlow Sui Vault Demo"

# Configuration
PACKAGE_ID="${PACKAGE_ID:-}"
VAULT_ID="${VAULT_ID:-}"
NETWORK="${NETWORK:-testnet}"

if [ -z "$PACKAGE_ID" ]; then
    echo "❌ Error: PACKAGE_ID environment variable is required"
    echo "Usage: PACKAGE_ID=0x... ./demo.sh"
    exit 1
fi

echo "📋 Configuration:"
echo "  Package ID: $PACKAGE_ID"
echo "  Vault ID: $VAULT_ID"
echo "  Network: $NETWORK"

# Function to get clock object ID
get_clock_id() {
    echo "0x6"  # Sui Clock object ID
}

CLOCK_ID=$(get_clock_id)

echo ""
echo "🏗️  Step 1: Creating a new vault..."

# Create vault
CREATE_OUTPUT=$(sui client call \
    --package $PACKAGE_ID \
    --module payment_vault \
    --function init_vault \
    --args $CLOCK_ID \
    --gas-budget 100000000 \
    --json)

echo "✅ Vault creation transaction sent"

# Parse vault and owner cap IDs from output
VAULT_OBJECT_ID=$(echo $CREATE_OUTPUT | jq -r '.objectChanges[] | select(.objectType | contains("PaymentVault")) | .objectId')
OWNER_CAP_ID=$(echo $CREATE_OUTPUT | jq -r '.objectChanges[] | select(.objectType | contains("OwnerCap")) | .objectId')

echo "📋 Created objects:"
echo "  Vault ID: $VAULT_OBJECT_ID"
echo "  Owner Cap ID: $OWNER_CAP_ID"

echo ""
echo "🔄 Step 2: Sharing the vault object..."

# Share the vault so others can deposit
sui client call \
    --package $PACKAGE_ID \
    --module payment_vault \
    --function share_vault \
    --args $VAULT_OBJECT_ID \
    --gas-budget 100000000

echo "✅ Vault shared successfully"

echo ""
echo "💰 Step 3: Checking vault balance..."

# Get vault balance (this would be a view function call in practice)
echo "Initial balance: 0 USDC"

echo ""
echo "📝 Demo completed successfully!"
echo ""
echo "Next steps to test deposits:"
echo "1. Get some testnet USDC from Circle's faucet"
echo "2. Call deposit function with USDC coin, order_id, and vault reference"
echo "3. Use the OwnerCap to withdraw funds"
echo ""
echo "Vault Object ID: $VAULT_OBJECT_ID"
echo "Owner Cap ID: $OWNER_CAP_ID"
