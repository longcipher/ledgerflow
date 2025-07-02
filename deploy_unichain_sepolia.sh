#!/bin/bash

# Unichain Sepolia Deployment Script for PaymentVault

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if required environment variables are set
check_env_vars() {
    if [ -z "$PRIVATE_KEY" ]; then
        echo -e "${RED}Error: PRIVATE_KEY environment variable is not set${NC}"
        echo "Please set it with: export PRIVATE_KEY=your_private_key"
        exit 1
    fi
    
    if [ -z "$ETHERSCAN_API_KEY" ]; then
        echo -e "${YELLOW}Warning: ETHERSCAN_API_KEY is not set. Contract verification will be skipped.${NC}"
    fi
}

# Deploy to Unichain Sepolia
deploy_to_unichain_sepolia() {
    echo -e "${YELLOW}Deploying PaymentVault to Unichain Sepolia...${NC}"
    
    # Unichain Sepolia USDC address (you may need to update this with the actual USDC address)
    USDC_ADDRESS="0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238" # Example USDC address, please verify
    
    forge script script/PaymentVault.s.sol:PaymentVaultScript \
        --rpc-url unichain_sepolia \
        --private-key $PRIVATE_KEY \
        --broadcast \
        --verify \
        --etherscan-api-key $ETHERSCAN_API_KEY \
        -vvvv
        
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅ Deployment to Unichain Sepolia successful!${NC}"
    else
        echo -e "${RED}❌ Deployment to Unichain Sepolia failed!${NC}"
        exit 1
    fi
}

# Verify contract on Unichain Sepolia (if deployment was done separately)
verify_contract() {
    if [ -z "$CONTRACT_ADDRESS" ]; then
        echo -e "${RED}Error: CONTRACT_ADDRESS environment variable is not set${NC}"
        echo "Please set it with: export CONTRACT_ADDRESS=your_deployed_contract_address"
        exit 1
    fi
    
    echo -e "${YELLOW}Verifying contract on Unichain Sepolia...${NC}"
    
    forge verify-contract \
        --rpc-url unichain_sepolia \
        --etherscan-api-key $ETHERSCAN_API_KEY \
        $CONTRACT_ADDRESS \
        src/PaymentVault.sol:PaymentVault
        
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅ Contract verification successful!${NC}"
    else
        echo -e "${RED}❌ Contract verification failed!${NC}"
        exit 1
    fi
}

# Main script
main() {
    echo -e "${GREEN}=== Unichain Sepolia Deployment Script ===${NC}"
    
    check_env_vars
    
    case "$1" in
        "deploy")
            deploy_to_unichain_sepolia
            ;;
        "verify")
            verify_contract
            ;;
        *)
            echo "Usage: $0 {deploy|verify}"
            echo ""
            echo "Commands:"
            echo "  deploy  - Deploy PaymentVault to Unichain Sepolia"
            echo "  verify  - Verify already deployed contract"
            echo ""
            echo "Required environment variables:"
            echo "  PRIVATE_KEY - Your wallet private key"
            echo "  ETHERSCAN_API_KEY - API key for contract verification (optional)"
            echo "  CONTRACT_ADDRESS - Deployed contract address (required for verify command)"
            echo ""
            echo "Example usage:"
            echo "  export PRIVATE_KEY=0x..."
            echo "  export ETHERSCAN_API_KEY=your_api_key"
            echo "  ./deploy_unichain_sepolia.sh deploy"
            exit 1
            ;;
    esac
}

main "$@"
