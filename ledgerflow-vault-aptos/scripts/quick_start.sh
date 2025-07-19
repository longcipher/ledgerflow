#!/bin/bash

# Quick Start Guide for LedgerFlow Payment Vault
# This script provides guided setup and testing

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}=== $1 ===${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_info() {
    echo -e "${CYAN}ℹ️  $1${NC}"
}

print_header "LedgerFlow Payment Vault - Quick Start Guide"
echo ""

# Check prerequisites
print_header "Checking Prerequisites"

# Check if Aptos CLI is installed
if ! command -v aptos &> /dev/null; then
    print_error "Aptos CLI not found!"
    echo "Please install it with:"
    echo "curl -fsSL \"https://aptos.dev/scripts/install_cli.py\" | python3"
    exit 1
fi
print_success "Aptos CLI found"

# Check if openssl is available
if ! command -v openssl &> /dev/null; then
    print_warning "OpenSSL not found - order ID generation will use fallback method"
else
    print_success "OpenSSL found"
fi

# Check if bc is available for calculations
if ! command -v bc &> /dev/null; then
    print_warning "BC calculator not found - some calculations may be simplified"
else
    print_success "BC calculator found"
fi

echo ""

# Check Aptos CLI profile
print_header "Checking Aptos CLI Configuration"

if ! aptos config show-profiles 2>/dev/null | grep -q "default"; then
    print_error "No Aptos CLI profile found!"
    echo ""
    print_info "Setting up Aptos CLI profile for testnet..."
    echo "Please follow the prompts to create a new account or import an existing one:"
    echo ""
    aptos init --network testnet
    echo ""
    print_success "Aptos CLI profile created!"
else
    print_success "Aptos CLI profile found"
fi

# Show current account
ACCOUNT=$(aptos config show-profiles | grep "account" | awk '{print $2}' | head -1)
echo "Current account: $ACCOUNT"
echo ""

# Contract information
VAULT_ADDRESS="0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846"
print_header "Contract Information"
echo "Vault Address: $VAULT_ADDRESS"
echo "Network: Aptos Testnet"
echo "Explorer: https://explorer.aptoslabs.com/account/$VAULT_ADDRESS?network=testnet"
echo ""

# Guide user through getting USDC
print_header "USDC Setup"
print_info "You need testnet USDC to interact with the vault."
echo ""
echo "Steps to get testnet USDC:"
echo "1. Visit: https://faucet.circle.com/"
echo "2. Connect your wallet or enter your address: $ACCOUNT"
echo "3. Request testnet USDC"
echo ""
read -p "Press Enter when you have obtained testnet USDC (or if you already have it)..."
echo ""

# Test vault interaction
print_header "Testing Vault Interaction"
echo ""

echo "Available test options:"
echo "1. Check vault status only"
echo "2. Deposit 1 USDC"
echo "3. Withdraw all funds (owner only)"
echo "4. Full test (deposit + withdraw)"
echo "5. Exit"
echo ""

while true; do
    read -p "Select option (1-5): " choice
    case $choice in
        1)
            ./scripts/vault_test.sh status
            break
            ;;
        2)
            echo ""
            print_info "Depositing 1 USDC to the vault..."
            ./scripts/deposit_1_usdc.sh
            break
            ;;
        3)
            echo ""
            print_warning "This will withdraw ALL funds from the vault!"
            print_warning "Only the vault owner can perform this operation."
            read -p "Are you sure you want to continue? (y/N): " confirm
            if [[ "$confirm" == "y" || "$confirm" == "Y" ]]; then
                ./scripts/withdraw_all_usdc.sh
            else
                echo "Withdrawal cancelled."
            fi
            break
            ;;
        4)
            echo ""
            print_info "Running full test (deposit 1 USDC then withdraw all)..."
            ./scripts/vault_test.sh full-test
            break
            ;;
        5)
            echo "Goodbye!"
            exit 0
            ;;
        *)
            print_error "Invalid option. Please select 1-5."
            ;;
    esac
done

echo ""
print_header "Next Steps"
echo ""
echo "You can now use the following scripts for further testing:"
echo ""
echo "• ./scripts/vault_test.sh status       - Check vault status"
echo "• ./scripts/vault_test.sh deposit      - Deposit 1 USDC"
echo "• ./scripts/vault_test.sh withdraw     - Withdraw all funds"
echo "• ./scripts/deposit_1_usdc.sh          - Simple deposit script"
echo "• ./scripts/withdraw_all_usdc.sh       - Simple withdrawal script"
echo ""
echo "For detailed documentation, see:"
echo "• ./scripts/README_SCRIPTS.md"
echo ""
print_success "Setup complete! Happy testing! 🚀"
