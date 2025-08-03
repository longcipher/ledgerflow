# LedgerFlow Sui Vault Makefile
# Provides convenient commands for development and deployment

.PHONY: help build test clean deploy demo format lint check-deps install-deps

# Default target
help:
	@echo "LedgerFlow Sui Vault - Available Commands:"
	@echo ""
	@echo "Development:"
	@echo "  build        - Build the Move package"
	@echo "  test         - Run Move tests"
	@echo "  clean        - Clean build artifacts"
	@echo "  format       - Format Move code (currently manual)"
	@echo "  lint         - Lint Move code"
	@echo ""
	@echo "Deployment:"
	@echo "  deploy       - Deploy to configured network"
	@echo "  demo         - Run interactive demo"
	@echo ""
	@echo "Setup:"
	@echo "  check-deps   - Check if required dependencies are installed"
	@echo "  install-deps - Install Sui CLI (requires manual steps)"
	@echo ""
	@echo "Environment Variables:"
	@echo "  NETWORK      - Target network (testnet/mainnet, default: testnet)"
	@echo "  PACKAGE_ID   - Deployed package ID (for demo)"

# Check if Sui CLI is installed
check-deps:
	@echo "🔍 Checking dependencies..."
	@command -v sui >/dev/null 2>&1 || { echo "❌ Sui CLI is not installed. Run 'make install-deps' for instructions."; exit 1; }
	@echo "✅ Sui CLI is installed: $$(sui --version)"
	@sui client active-env >/dev/null 2>&1 || { echo "⚠️  Sui client not configured. Run 'sui client' to set up."; }
	@echo "✅ Dependencies check completed"

# Install dependencies (provides instructions)
install-deps:
	@echo "📦 Installing Sui CLI..."
	@echo ""
	@echo "Please follow these steps:"
	@echo "1. Install Sui CLI following the official guide:"
	@echo "   https://docs.sui.io/guides/developer/getting-started/sui-install"
	@echo ""
	@echo "2. Configure Sui client:"
	@echo "   sui client"
	@echo ""
	@echo "3. Verify installation:"
	@echo "   make check-deps"

# Build the Move package
build: check-deps
	@echo "🔨 Building Move package..."
	sui move build
	@echo "✅ Build completed"

# Run tests
test: check-deps
	@echo "🧪 Running Move tests..."
	sui move test
	@echo "✅ Tests completed"

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	rm -rf build/
	rm -rf .sui/
	@echo "✅ Clean completed"

# Format Move code (manual for now)
format:
	@echo "📝 Move formatter not available yet"
	@echo "Please format code manually following Move style guidelines"

# Lint Move code
lint: build
	@echo "🔍 Linting Move code..."
	@echo "✅ Move code syntax validated through build"

# Deploy to network
deploy: check-deps build
	@echo "🚀 Deploying to network..."
	./scripts/deploy.sh
	@echo "✅ Deployment completed"

# Run demo
demo: check-deps
	@echo "🎮 Running demo..."
	@if [ -z "$(PACKAGE_ID)" ]; then \
		echo "❌ PACKAGE_ID environment variable is required"; \
		echo "Usage: PACKAGE_ID=0x... make demo"; \
		exit 1; \
	fi
	./scripts/demo.sh
	@echo "✅ Demo completed"

# Development workflow
dev: clean build test
	@echo "✅ Development workflow completed"

# Quick check (build + test)
check: build test
	@echo "✅ Quick check completed"

# Show current configuration
config:
	@echo "📋 Current Configuration:"
	@echo "  Network: $${NETWORK:-testnet}"
	@echo "  Package ID: $${PACKAGE_ID:-<not set>}"
	@echo "  Sui Version: $$(sui --version 2>/dev/null || echo 'not installed')"
	@echo "  Active Env: $$(sui client active-env 2>/dev/null || echo 'not configured')"
	@echo "  Active Address: $$(sui client active-address 2>/dev/null || echo 'not configured')"
