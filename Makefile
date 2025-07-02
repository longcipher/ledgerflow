# Makefile for PaymentVault deployment

.PHONY: help build test clean deploy-unichain-sepolia verify-unichain-sepolia

# Default target
help:
	@echo "Available commands:"
	@echo "  build                 - Build the smart contracts"
	@echo "  test                  - Run all tests"
	@echo "  clean                 - Clean build artifacts"
	@echo "  deploy-unichain-sepolia - Deploy to Unichain Sepolia testnet"
	@echo "  verify-unichain-sepolia - Verify contract on Unichain Sepolia"
	@echo ""
	@echo "Required environment variables:"
	@echo "  PRIVATE_KEY          - Your wallet private key"
	@echo "  ETHERSCAN_API_KEY    - API key for contract verification (optional)"
	@echo "  CONTRACT_ADDRESS     - Deployed contract address (for verification)"

# Build contracts
build:
	forge build

# Run tests
test:
	forge test -vvv

# Clean build artifacts
clean:
	forge clean

# Deploy to Unichain Sepolia
deploy-unichain-sepolia:
	@echo "Deploying to Unichain Sepolia..."
	@if [ -z "$(PRIVATE_KEY)" ]; then \
		echo "Error: PRIVATE_KEY environment variable is required"; \
		exit 1; \
	fi
	forge script script/PaymentVault.s.sol:PaymentVaultScript \
		--rpc-url unichain_sepolia \
		--private-key $(PRIVATE_KEY) \
		--broadcast \
		--verify \
		--etherscan-api-key $(ETHERSCAN_API_KEY) \
		-vvvv

# Verify contract on Unichain Sepolia
verify-unichain-sepolia:
	@echo "Verifying contract on Unichain Sepolia..."
	@if [ -z "$(CONTRACT_ADDRESS)" ]; then \
		echo "Error: CONTRACT_ADDRESS environment variable is required"; \
		exit 1; \
	fi
	@if [ -z "$(ETHERSCAN_API_KEY)" ]; then \
		echo "Error: ETHERSCAN_API_KEY environment variable is required"; \
		exit 1; \
	fi
	forge verify-contract \
		--rpc-url unichain_sepolia \
		--etherscan-api-key $(ETHERSCAN_API_KEY) \
		$(CONTRACT_ADDRESS) \
		src/PaymentVault.sol:PaymentVault

# Deploy without verification
deploy-unichain-sepolia-no-verify:
	@echo "Deploying to Unichain Sepolia (without verification)..."
	@if [ -z "$(PRIVATE_KEY)" ]; then \
		echo "Error: PRIVATE_KEY environment variable is required"; \
		exit 1; \
	fi
	forge script script/PaymentVault.s.sol:PaymentVaultScript \
		--rpc-url unichain_sepolia \
		--private-key $(PRIVATE_KEY) \
		--broadcast \
		-vvvv
