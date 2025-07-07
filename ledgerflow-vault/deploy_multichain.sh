#!/bin/bash

# Multi-chain Deterministic Deployment Script for PaymentVault
# This script deploys the PaymentVault contract to multiple EVM-compatible chains
# with the same contract address using CREATE2

set -e

# Configuration
SCRIPT_NAME="DeployDeterministic.s.sol"
PRIVATE_KEY_VAR="PRIVATE_KEY"
INITIAL_OWNER_VAR="INITIAL_OWNER"

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if environment variable is set
check_env_var() {
    if [ -z "${!1}" ]; then
        print_error "Environment variable $1 is not set"
        return 1
    fi
    return 0
}

# Function to deploy to a specific chain
deploy_to_chain() {
    local chain_name=$1
    local rpc_url=$2
    local usdc_address=$3
    local verify_flag=$4

    print_status "Deploying to $chain_name..."
    print_status "RPC URL: $rpc_url"
    print_status "USDC Address: $usdc_address"

    # Set environment variables for the deployment
    export USDC_TOKEN_ADDRESS=$usdc_address

    # Build the forge command
    local forge_cmd="forge script script/$SCRIPT_NAME --rpc-url $rpc_url --private-key \$$PRIVATE_KEY_VAR --broadcast"
    
    if [ "$verify_flag" = "true" ]; then
        forge_cmd="$forge_cmd --verify"
    fi

    # Execute deployment
    if eval $forge_cmd; then
        print_success "Successfully deployed to $chain_name"
    else
        print_error "Failed to deploy to $chain_name"
        return 1
    fi

    echo ""
}

# Function to predict addresses before deployment
predict_addresses() {
    local deployer_address=$1
    
    print_status "Predicting deployment addresses..."
    print_status "Deployer: $deployer_address"
    print_status "Salt: PaymentVault_v1.0.0"
    
    # You can add address prediction logic here
    # For now, we'll rely on the script's prediction function
    echo ""
}

# Main deployment function
main() {
    print_status "Starting multi-chain deployment of PaymentVault..."

    # Check required environment variables
    if ! check_env_var "$PRIVATE_KEY_VAR"; then
        print_error "Please set your private key: export PRIVATE_KEY=0x..."
        exit 1
    fi

    # Check if we have a deployer address (optional, will use msg.sender)
    if [ -n "${INITIAL_OWNER}" ]; then
        print_status "Initial owner: $INITIAL_OWNER"
    else
        print_warning "INITIAL_OWNER not set, will use deployer address"
    fi

    # Get deployer address for prediction (optional)
    if [ -n "${DEPLOYER_ADDRESS}" ]; then
        predict_addresses "$DEPLOYER_ADDRESS"
    fi

    print_status "Starting deployments..."

    # Chain configurations: chain_name:rpc_key:usdc_address:verify
    # Note: Update USDC addresses for each chain as needed
    
    # Testnets (recommended for testing first)
    deploy_to_chain "Sepolia" "sepolia" "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238" "true"
    deploy_to_chain "Polygon Mumbai" "polygon_mumbai" "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174" "false"
    deploy_to_chain "Arbitrum Sepolia" "arbitrum_sepolia" "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d" "false"
    deploy_to_chain "Optimism Sepolia" "optimism_sepolia" "0x5fd84259d66Cd46123540766Be93DFE6D43130D7" "false"
    deploy_to_chain "Base Sepolia" "base_sepolia" "0x036CbD53842c5426634e7929541eC2318f3dCF7e" "false"
    deploy_to_chain "Unichain Sepolia" "unichain_sepolia" "0x1234567890123456789012345678901234567890" "true"

    # Uncomment the following for mainnet deployments
    # deploy_to_chain "Ethereum" "ethereum" "0xA0b86a33E6417c5DeF6Ca95E2B6b81b9c8C06b6" "true"
    # deploy_to_chain "Polygon" "polygon" "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174" "true"
    # deploy_to_chain "Arbitrum" "arbitrum" "0xaf88d065e77c8cC2239327C5EDb3A432268e5831" "true"
    # deploy_to_chain "Optimism" "optimism" "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85" "true"
    # deploy_to_chain "Base" "base" "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913" "true"
    # deploy_to_chain "Avalanche" "avalanche" "0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E" "true"
    # deploy_to_chain "BSC" "bsc" "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d" "true"

    print_success "All deployments completed!"
    print_status "The contract should have the same address on all chains where deployment succeeded."
    print_warning "Please verify the addresses manually and update your documentation."
}

# Help function
show_help() {
    echo "Multi-chain Deterministic Deployment Script for PaymentVault"
    echo ""
    echo "Required environment variables:"
    echo "  PRIVATE_KEY        - Your private key for deployment"
    echo ""
    echo "Optional environment variables:"
    echo "  INITIAL_OWNER      - Initial owner address (defaults to deployer)"
    echo "  DEPLOYER_ADDRESS   - For address prediction (optional)"
    echo ""
    echo "Usage:"
    echo "  $0                 - Deploy to all configured chains"
    echo "  $0 --help         - Show this help message"
    echo ""
    echo "Example:"
    echo "  export PRIVATE_KEY=0x1234..."
    echo "  export INITIAL_OWNER=0xabcd..."
    echo "  $0"
}

# Parse command line arguments
case "${1:-}" in
    --help|-h)
        show_help
        exit 0
        ;;
    "")
        main
        ;;
    *)
        print_error "Unknown option: $1"
        show_help
        exit 1
        ;;
esac
