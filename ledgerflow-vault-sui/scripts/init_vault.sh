#!/bin/bash

# Initialize a new USDC payment vault
# This script creates a vault and shares it publicly, while transferring the owner capability to the deployer

echo "Initializing USDC Payment Vault..."

PACKAGE_ID="0xd0a37165e44917ac53a2429d50b0edc26f8be103671e5a68167714a897f4d376"
USDC_TYPE="0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC"
DEPLOYER="0xcd8369e1a8ae681fb05660ffe9811872daff3f6946a4981c2e573a0627c3a877"
CLOCK="0x6"

# Create vault and get owner capability
result=$(sui client call \
    --package $PACKAGE_ID \
    --module payment_vault \
    --function init_vault \
    --type-args $USDC_TYPE \
    --args $CLOCK \
    --gas-budget 100000000 \
    --json 2>/dev/null)

if [ $? -eq 0 ]; then
    echo "✅ Vault created successfully!"
    
    # Extract vault ID and owner cap ID from the transaction result
    vault_id=$(echo $result | jq -r '.objectChanges[] | select(.type == "created" and (.objectType | contains("PaymentVault"))) | .objectId')
    owner_cap_id=$(echo $result | jq -r '.objectChanges[] | select(.type == "created" and (.objectType | contains("OwnerCap"))) | .objectId')
    
    echo "🏦 Vault ID: $vault_id"
    echo "🔑 Owner Cap ID: $owner_cap_id"
    
    # Share the vault object so it can be used publicly
    echo "Sharing vault object..."
    share_result=$(sui client call \
        --package 0x2 \
        --module transfer \
        --function share_object \
        --type-args "$PACKAGE_ID::payment_vault::PaymentVault<$USDC_TYPE>" \
        --args $vault_id \
        --gas-budget 100000000 \
        --json 2>/dev/null)
    
    if [ $? -eq 0 ]; then
        echo "✅ Vault shared successfully!"
        
        # Save deployment info
        cat > vault_deployment.json << EOF
{
  "network": "testnet",
  "packageId": "$PACKAGE_ID",
  "vaultId": "$vault_id",
  "ownerCapId": "$owner_cap_id",
  "usdcType": "$USDC_TYPE",
  "deployer": "$DEPLOYER",
  "deployedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
        echo "📄 Deployment info saved to vault_deployment.json"
    else
        echo "❌ Failed to share vault"
        exit 1
    fi
else
    echo "❌ Failed to create vault"
    exit 1
fi
