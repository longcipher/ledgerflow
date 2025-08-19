#!/bin/bash

# Comprehensive test script for LedgerFlow Payment Vault interactions
# This script provides utilities to check vault status, deposit, and withdraw

set -e

# Configuration
VAULT_ADDRESS="0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846"
USDC_METADATA_ADDRESS="0x69091fbab5f7d635ee7ac5098cf0c1efbe31d68fec0f2cd565e8d168daf52832"
NETWORK="testnet"
API_URL="https://api.testnet.aptoslabs.com/v1"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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

# Function to check vault status
check_vault_status() {
    print_header "Vault Status Check"
    echo "Vault Address: $VAULT_ADDRESS"
    echo ""

    # Check if vault exists
    echo "Checking if vault exists..."
    EXISTS_RESULT=$(aptos move view \
        --function-id ${VAULT_ADDRESS}::payment_vault::vault_exists \
        --args address:$VAULT_ADDRESS \
        --url $API_URL 2>/dev/null || echo "ERROR")

    if [[ $EXISTS_RESULT == *"ERROR"* ]]; then
        print_error "Failed to check vault existence"
        return 1
    fi

    if [[ $EXISTS_RESULT == *"true"* ]]; then
        print_success "Vault exists"
    else
        print_error "Vault does not exist"
        return 1
    fi

    # Get vault owner
    echo "Getting vault owner..."
    OWNER_RESULT=$(aptos move view \
        --function-id ${VAULT_ADDRESS}::payment_vault::get_owner \
        --args address:$VAULT_ADDRESS \
        --url $API_URL 2>/dev/null || echo "ERROR")

    if [[ $OWNER_RESULT != *"ERROR"* ]]; then
        VAULT_OWNER=$(echo $OWNER_RESULT | grep -o '0x[a-fA-F0-9]*' | head -1)
        echo "Vault Owner: $VAULT_OWNER"
    else
        print_error "Failed to get vault owner"
    fi

    # Get vault balance
    echo "Getting vault balance..."
    BALANCE_RESULT=$(aptos move view \
        --function-id ${VAULT_ADDRESS}::payment_vault::get_balance \
        --args address:$VAULT_ADDRESS \
        --url $API_URL 2>/dev/null || echo "ERROR")

    if [[ $BALANCE_RESULT != *"ERROR"* ]]; then
        VAULT_BALANCE=$(echo $BALANCE_RESULT | grep -o '[0-9]*' | head -1)
        VAULT_BALANCE_USDC=$(echo "scale=6; $VAULT_BALANCE / 1000000" | bc -l)
        echo "Vault Balance: $VAULT_BALANCE micro-USDC ($VAULT_BALANCE_USDC USDC)"
    else
        print_error "Failed to get vault balance"
    fi

    # Get deposit count
    echo "Getting deposit count..."
    COUNT_RESULT=$(aptos move view \
        --function-id ${VAULT_ADDRESS}::payment_vault::get_deposit_count \
        --args address:$VAULT_ADDRESS \
        --url $API_URL 2>/dev/null || echo "ERROR")

    if [[ $COUNT_RESULT != *"ERROR"* ]]; then
        DEPOSIT_COUNT=$(echo $COUNT_RESULT | grep -o '[0-9]*' | head -1)
        echo "Total Deposits: $DEPOSIT_COUNT"
    else
        print_error "Failed to get deposit count"
    fi

    echo ""
}

# Function to check user USDC balance
check_user_balance() {
    local account=$1
    print_header "User USDC Balance Check"
    echo "Account: $account"
    
    # This is a simplified check - in practice you'd need to query the fungible asset balance
    echo "Checking account resources..."
    RESOURCES=$(aptos account list --query resources --account $account --url $API_URL 2>/dev/null || echo "ERROR")
    
    if [[ $RESOURCES == *"$USDC_METADATA_ADDRESS"* ]]; then
        print_success "Account has USDC resources"
    else
        print_warning "No USDC resources found. You may need to acquire testnet USDC."
        echo "Get testnet USDC from: https://faucet.circle.com/"
    fi
    echo ""
}

# Function to deposit USDC
deposit_usdc() {
    local amount=${1:-1000000}  # Default 1 USDC
    
    print_header "Deposit USDC"
    
    # Generate unique order ID
    ORDER_ID="0x$(openssl rand -hex 32)"
    AMOUNT_USDC=$(echo "scale=6; $amount / 1000000" | bc -l)
    
    echo "Amount: $amount micro-USDC ($AMOUNT_USDC USDC)"
    echo "Order ID: $ORDER_ID"
    echo ""
    
    # Execute deposit
    echo "Executing deposit transaction..."
    aptos move run \
        --function-id ${VAULT_ADDRESS}::payment_vault::deposit \
        --args address:$VAULT_ADDRESS \
        --args hex:$ORDER_ID \
        --args u64:$amount \
        --url $API_URL \
        --assume-yes

    if [ $? -eq 0 ]; then
        print_success "Deposit completed successfully!"
        echo "Order ID: $ORDER_ID"
    else
        print_error "Deposit failed!"
        return 1
    fi
    echo ""
}

# Function to withdraw all USDC
withdraw_all() {
    local recipient=${1:-}
    
    print_header "Withdraw All USDC"
    
    # Get current account if no recipient specified
    if [[ -z "$recipient" ]]; then
        ACCOUNT=$(aptos config show-profiles | grep '"account":' | head -1 | awk -F'"' '{print $4}')
        recipient=$ACCOUNT
    fi
    
    echo "Recipient: $recipient"
    echo ""
    
    # Execute withdrawal
    echo "Executing withdraw_all transaction..."
    aptos move run \
        --function-id ${VAULT_ADDRESS}::payment_vault::withdraw_all \
        --args address:$VAULT_ADDRESS \
        --args address:$recipient \
        --url $API_URL \
        --assume-yes

    if [ $? -eq 0 ]; then
        print_success "Withdrawal completed successfully!"
    else
        print_error "Withdrawal failed!"
        return 1
    fi
    echo ""
}

# Main function
main() {
    print_header "LedgerFlow Payment Vault Test Script"
    echo "Network: $NETWORK"
    echo "Vault: $VAULT_ADDRESS"
    echo ""

    # Check if Aptos CLI is available
    if ! command -v aptos &> /dev/null; then
        print_error "Aptos CLI not found. Please install it first."
        exit 1
    fi

    # Check if profile exists
    if ! aptos config show-profiles 2>/dev/null | grep -q "default"; then
        print_error "No Aptos CLI profile found. Please run 'aptos init' first."
        exit 1
    fi

    ACCOUNT=$(aptos config show-profiles | grep '"account":' | head -1 | awk -F'"' '{print $4}')
    echo "Using account: $ACCOUNT"
    echo ""

    case "${1:-status}" in
        "status")
            check_vault_status
            check_user_balance $ACCOUNT
            ;;
        "deposit")
            check_vault_status
            deposit_usdc ${2:-1000000}
            ;;
        "withdraw")
            check_vault_status
            withdraw_all ${2:-}
            ;;
        "full-test")
            check_vault_status
            check_user_balance $ACCOUNT
            echo "Press Enter to deposit 1 USDC..."
            read
            deposit_usdc 1000000
            sleep 2
            check_vault_status
            echo "Press Enter to withdraw all..."
            read
            withdraw_all
            sleep 2
            check_vault_status
            ;;
        *)
            echo "Usage: $0 [status|deposit|withdraw|full-test]"
            echo ""
            echo "Commands:"
            echo "  status      - Check vault status and balances"
            echo "  deposit     - Deposit 1 USDC (or specify amount as 2nd arg)"
            echo "  withdraw    - Withdraw all USDC (optionally specify recipient as 2nd arg)"
            echo "  full-test   - Run complete deposit and withdrawal test"
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"
