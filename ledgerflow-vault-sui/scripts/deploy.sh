#!/bin/bash

# LedgerFlow Sui Vault Deployment Script
# This script deploys the payment vault contract to Sui blockchain

set -e

echo "🚀 Starting LedgerFlow Sui Vault deployment..."

# Configuration
NETWORK="${NETWORK:-testnet}"
GAS_BUDGET="${GAS_BUDGET:-100000000}"

echo "📋 Configuration:"
echo "  Network: $NETWORK"
echo "  Gas Budget: $GAS_BUDGET"

# Check if sui CLI is installed
if ! command -v sui &> /dev/null; then
    echo "❌ Error: sui CLI is not installed or not in PATH"
    echo "Please install Sui CLI: https://docs.sui.io/guides/developer/getting-started/sui-install"
    exit 1
fi

# Check if sui client is configured
if ! sui client active-env &> /dev/null; then
    echo "❌ Error: Sui client is not configured"
    echo "Please run: sui client"
    exit 1
fi

echo "✅ Sui CLI is configured"

# Build the package
echo "🔨 Building the package..."
sui move build

if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

echo "✅ Build successful"

# Deploy the package
echo "📦 Deploying package to $NETWORK..."

DEPLOY_OUTPUT=$(sui client publish --gas-budget $GAS_BUDGET --json)

if [ $? -ne 0 ]; then
    echo "❌ Deployment failed"
    exit 1
fi

echo "✅ Deployment successful!"

# Parse deployment output
PACKAGE_ID=$(echo $DEPLOY_OUTPUT | jq -r '.objectChanges[] | select(.type == "published") | .packageId')

echo ""
echo "📋 Deployment Summary:"
echo "  Package ID: $PACKAGE_ID"
echo "  Network: $NETWORK"

# Save deployment info
DEPLOYMENT_FILE="deployments/${NETWORK}.json"
mkdir -p deployments

cat > $DEPLOYMENT_FILE << EOF
{
  "network": "$NETWORK",
  "packageId": "$PACKAGE_ID",
  "deployedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "deployer": "$(sui client active-address)",
  "gasUsed": $(echo $DEPLOY_OUTPUT | jq '.effects.gasUsed.computationCost + .effects.gasUsed.storageCost + .effects.gasUsed.storageRebate')
}
EOF

echo "  Deployment info saved to: $DEPLOYMENT_FILE"

echo ""
echo "🎉 LedgerFlow Sui Vault deployed successfully!"
echo ""
echo "Next steps:"
echo "1. Create a vault: sui client call --package $PACKAGE_ID --module payment_vault --function init_vault"
echo "2. Share the vault object for public access"
echo "3. Keep the OwnerCap for administrative functions"
echo ""
echo "📖 For more information, see the README.md file"
