#!/bin/bash

# LedgerFlow CLI Example Script
# This script demonstrates how to use the LedgerFlow CLI tool

# Configuration
RPC_URL="https://sepolia.unichain.org"  # Unichain Sepolia testnet
PRIVATE_KEY="YOUR_PRIVATE_KEY_HERE"
CONTRACT_ADDRESS="YOUR_VAULT_CONTRACT_ADDRESS_HERE"
ORDER_ID="0x1234567890123456789012345678901234567890123456789012345678901234"
AMOUNT="1000000"  # 1 USDC (6 decimals)
DEADLINE=$(($(date +%s) + 3600))  # 1 hour from now

# CLI binary path
CLI="./target/release/ledgerflow-eth-cli"

echo "🚀 LedgerFlow CLI Example Usage"
echo "================================="

# Check if CLI binary exists
if [ ! -f "$CLI" ]; then
    echo "❌ CLI binary not found. Building..."
    cargo build --release
    if [ $? -ne 0 ]; then
        echo "❌ Build failed!"
        exit 1
    fi
fi

echo "📖 Available commands:"
$CLI --help

echo -e "\n🔍 Deposit command help:"
$CLI deposit --help

echo -e "\n🔍 Deposit with permit command help:"
$CLI deposit-with-permit --help

echo -e "\n🔍 Withdraw command help:"
$CLI withdraw --help

echo -e "\n⚠️  To use the CLI with real transactions, update the variables above:"
echo "   - Set your RPC_URL"
echo "   - Set your PRIVATE_KEY"
echo "   - Set your CONTRACT_ADDRESS"
echo "   - Customize ORDER_ID and AMOUNT as needed"

echo -e "\n📝 Example commands (DO NOT RUN without proper configuration):"

echo -e "\n# Standard deposit (requires prior USDC approval):"
echo "$CLI deposit \\"
echo "  --rpc-url \"$RPC_URL\" \\"
echo "  --private-key \"$PRIVATE_KEY\" \\"
echo "  --contract-address \"$CONTRACT_ADDRESS\" \\"
echo "  --order-id \"$ORDER_ID\" \\"
echo "  --amount $AMOUNT"

echo -e "\n# Permit deposit (gas efficient, no prior approval needed):"
echo "$CLI deposit-with-permit \\"
echo "  --rpc-url \"$RPC_URL\" \\"
echo "  --private-key \"$PRIVATE_KEY\" \\"
echo "  --contract-address \"$CONTRACT_ADDRESS\" \\"
echo "  --order-id \"$ORDER_ID\" \\"
echo "  --amount $AMOUNT \\"
echo "  --deadline $DEADLINE"

echo -e "\n# Withdraw (owner only):"
echo "$CLI withdraw \\"
echo "  --rpc-url \"$RPC_URL\" \\"
echo "  --private-key \"$PRIVATE_KEY\" \\"
echo "  --contract-address \"$CONTRACT_ADDRESS\""

echo -e "\n✅ CLI tool is ready for use!"
echo "⚡ For actual transactions, make sure to:"
echo "   1. Have sufficient USDC balance"
echo "   2. Have ETH for gas fees"
echo "   3. Use correct contract addresses"
echo "   4. Test on testnets first"
