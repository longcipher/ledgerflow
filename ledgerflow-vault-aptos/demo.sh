#!/bin/bash

# Demo script showing basic vault operations
# This script demonstrates deposit and withdrawal in a safe manner

set -e

VAULT_ADDRESS="0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846"

echo "=== LedgerFlow Payment Vault Demo ==="
echo "Vault Address: $VAULT_ADDRESS"
echo ""

# Check if we can connect to the vault
echo "🔍 Checking vault status..."
./scripts/vault_test.sh status

echo ""
echo "📝 Ready to test deposit and withdrawal operations."
echo ""
echo "To test deposit (1 USDC):"
echo "  ./scripts/deposit_1_usdc.sh"
echo ""
echo "To test withdrawal (all funds, owner only):"
echo "  ./scripts/withdraw_all_usdc.sh"
echo ""
echo "To run comprehensive tests:"
echo "  ./scripts/vault_test.sh full-test"
echo ""
echo "To start interactive setup:"
echo "  ./scripts/quick_start.sh"
echo ""
echo "✅ Demo setup complete!"
