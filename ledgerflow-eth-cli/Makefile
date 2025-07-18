# LedgerFlow CLI Makefile

.PHONY: build test clean install dev lint fmt check help

# Build the project
build:
	cargo build --release

# Build for development
dev:
	cargo build

# Run tests
test:
	cargo test

# Clean build artifacts
clean:
	cargo clean

# Install the CLI globally
install: build
	cargo install --path .

# Run linter
lint:
	cargo clippy -- -D warnings

# Format code
fmt:
	cargo fmt

# Check code (lint + format check)
check:
	cargo fmt --check
	cargo clippy -- -D warnings

# Run with example parameters (requires environment variables)
example-deposit:
	@echo "Running example deposit..."
	@./target/release/ledgerflow-eth-cli deposit \
		--rpc-url "${RPC_URL}" \
		--private-key "${PRIVATE_KEY}" \
		--contract-address "${VAULT_ADDRESS}" \
		--order-id "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef" \
		--amount 1000000

example-permit:
	@echo "Running example permit deposit..."
	@./target/release/ledgerflow-eth-cli deposit-with-permit \
		--rpc-url "${RPC_URL}" \
		--private-key "${PRIVATE_KEY}" \
		--contract-address "${VAULT_ADDRESS}" \
		--order-id "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef" \
		--amount 1000000 \
		--deadline $$(date -d "+1 hour" +%s)

example-withdraw:
	@echo "Running example withdrawal..."
	@./target/release/ledgerflow-eth-cli withdraw \
		--rpc-url "${RPC_URL}" \
		--private-key "${PRIVATE_KEY}" \
		--contract-address "${VAULT_ADDRESS}"

# Help
help:
	@echo "Available targets:"
	@echo "  build         - Build the project in release mode"
	@echo "  dev           - Build for development"
	@echo "  test          - Run tests"
	@echo "  clean         - Clean build artifacts"
	@echo "  install       - Install CLI globally"
	@echo "  lint          - Run linter"
	@echo "  fmt           - Format code"
	@echo "  check         - Check code (lint + format)"
	@echo "  example-*     - Run example commands (requires env vars)"
	@echo "  help          - Show this help"
	@echo ""
	@echo "Required environment variables for examples:"
	@echo "  RPC_URL       - RPC endpoint URL"
	@echo "  PRIVATE_KEY   - Wallet private key"
	@echo "  VAULT_ADDRESS - PaymentVault contract address"
