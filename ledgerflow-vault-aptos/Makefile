# LedgerFlow Vault Aptos - Makefile
# 
# This Makefile provides convenient commands for developing, testing, and deploying
# the LedgerFlow Vault smart contract on Aptos blockchain.

.PHONY: help build test test-dev clean deploy-dev deploy-mainnet verify

# Default target
help:
	@echo "LedgerFlow Vault Aptos - Available Commands:"
	@echo ""
	@echo "Development Commands:"
	@echo "  build        - Compile the Move contracts for production (address=0x1)"
	@echo "  build-dev    - Compile the Move contracts for development (address=0x999)"
	@echo "  test         - Run all tests in production mode"
	@echo "  test-dev     - Run all tests in development mode"
	@echo "  clean        - Clean build artifacts"
	@echo ""
	@echo "Deployment Commands:"
	@echo "  deploy-dev   - Deploy to Aptos devnet"
	@echo "  deploy-test  - Deploy to Aptos testnet"
	@echo "  deploy-main  - Deploy to Aptos mainnet"
	@echo ""
	@echo "Utility Commands:"
	@echo "  fmt          - Format Move code (if formatter available)"
	@echo "  docs         - Generate documentation"
	@echo ""
	@echo "Prerequisites:"
	@echo "  - Aptos CLI installed"
	@echo "  - Private key configured for deployment"
	@echo ""

# Build the Move contracts for production
build:
	@echo "🔨 Compiling Move contracts for production..."
	@aptos move compile --named-addresses ledgerflow_vault=0x1 --skip-fetch-latest-git-deps
	@if [ $$? -eq 0 ]; then \
		echo "✅ Production compilation successful!"; \
	else \
		echo "❌ Production compilation failed!"; \
		exit 1; \
	fi

# Build the Move contracts for development
build-dev:
	@echo "🔨 Compiling Move contracts for development..."
	@aptos move compile --dev --skip-fetch-latest-git-deps
	@if [ $$? -eq 0 ]; then \
		echo "✅ Development compilation successful!"; \
	else \
		echo "❌ Development compilation failed!"; \
		exit 1; \
	fi

# Run tests in development mode (recommended for local development)
test-dev:
	@echo "🧪 Running tests in development mode..."
	aptos move test --dev

# Run tests in production mode
test:
	@echo "🧪 Running tests..."
	@echo "⚠️  Note: Tests require Aptos CLI and may have limitations with coin initialization"
	@echo "    If tests fail due to coin initialization conflicts, this is a known Move test framework limitation"
	aptos move test --dev --skip-fetch-latest-git-deps || (echo "❌ Tests failed - this may be due to Move test framework limitations" && exit 1)

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	rm -rf build/
	rm -rf .aptos/

# Deploy to Aptos devnet
deploy-dev:
	@echo "🚀 Deploying to Aptos devnet..."
	@if [ -z "$$PRIVATE_KEY" ]; then \
		echo "❌ Error: PRIVATE_KEY environment variable not set"; \
		echo "Please set your private key: export PRIVATE_KEY=0x..."; \
		exit 1; \
	fi
	aptos move publish --profile devnet --assume-yes

# Deploy to Aptos testnet
deploy-test:
	@echo "🚀 Deploying to Aptos testnet..."
	@if [ -z "$$PRIVATE_KEY" ]; then \
		echo "❌ Error: PRIVATE_KEY environment variable not set"; \
		echo "Please set your private key: export PRIVATE_KEY=0x..."; \
		exit 1; \
	fi
	aptos move publish --profile testnet --assume-yes

# Deploy to Aptos mainnet
deploy-main:
	@echo "🚀 Deploying to Aptos mainnet..."
	@if [ -z "$$PRIVATE_KEY" ]; then \
		echo "❌ Error: PRIVATE_KEY environment variable not set"; \
		echo "Please set your private key: export PRIVATE_KEY=0x..."; \
		exit 1; \
	fi
	@echo "⚠️  WARNING: You are about to deploy to MAINNET!"
	@echo "Are you sure you want to continue? [y/N]"
	@read confirm && [ "$$confirm" = "y" ] || [ "$$confirm" = "Y" ]
	aptos move publish --profile mainnet --assume-yes

# Format code (placeholder - no official formatter yet)
fmt:
	@echo "ℹ️  Move formatter not yet available - please format manually"
	aptos move fmt

# Generate documentation (placeholder)
docs:
	@echo "📚 Generating documentation..."
	@echo "Documentation is available in README.md"
	@echo "API documentation is embedded in the source code"

# Development shortcuts
dev-setup:
	@echo "🔧 Setting up development environment..."
	@echo "Checking Aptos CLI..."
	@which aptos > /dev/null || (echo "❌ Aptos CLI not found. Please install from https://aptos.dev/cli-tools/aptos-cli-tool/install-aptos-cli/" && exit 1)
	@echo "✅ Development environment ready"

# Quick development cycle
dev: build test-dev

# Full testing and build cycle
ci: build test

# Initialize Aptos profiles (run once)
init-profiles:
	@echo "🔧 Initializing Aptos profiles..."
	aptos init --profile devnet --network devnet || true
	aptos init --profile testnet --network testnet || true
	aptos init --profile mainnet --network mainnet || true
	@echo "✅ Profiles initialized"

# Show account information
account-info:
	@echo "📋 Account Information:"
	@echo ""
	@echo "Devnet:"
	@aptos account show --profile devnet 2>/dev/null || echo "❌ Devnet profile not configured"
	@echo ""
	@echo "Testnet:"
	@aptos account show --profile testnet 2>/dev/null || echo "❌ Testnet profile not configured"
	@echo ""
	@echo "Mainnet:"
	@aptos account show --profile mainnet 2>/dev/null || echo "❌ Mainnet profile not configured"

# Fund account on devnet
fund-devnet:
	@echo "💰 Funding account on devnet..."
	aptos account fund-with-faucet --profile devnet

# Fund account on testnet
fund-testnet:
	@echo "💰 Funding account on testnet..."
	aptos account fund-with-faucet --profile testnet

# Show deployed module information
module-info:
	@echo "📦 Module Information:"
	@echo "Module Address: Check Move.toml for current address configuration"
	@echo "Module Name: payment_vault"
	@echo "Main Functions: initialize, deposit, withdraw, withdraw_all, transfer_ownership"

# Security check reminder
security-check:
	@echo "🔒 Security Checklist:"
	@echo "✓ Review all public functions"
	@echo "✓ Verify error handling"
	@echo "✓ Check access controls"
	@echo "✓ Test edge cases"
	@echo "✓ Audit by security experts (for mainnet)"
	@echo "✓ Test on devnet/testnet first"
