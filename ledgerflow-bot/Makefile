# LedgerFlow Bot Makefile

.PHONY: help build run test clean setup migration-create migration-run docker-build docker-run

help: ## Show this help message
	@echo "Available commands:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

build: ## Build the project
	cargo build

build-release: ## Build the project in release mode
	cargo build --release

run: ## Run the bot in development mode
	cargo run -- start

run-release: ## Run the bot in release mode
	cargo run --release -- start

test: ## Run tests
	cargo test

test-verbose: ## Run tests with verbose output
	cargo test -- --nocapture

clean: ## Clean build artifacts
	cargo clean

setup: ## Setup development environment
	@echo "Setting up development environment..."
	@if [ ! -f config.yaml ]; then cp config.yaml.example config.yaml; echo "Created config.yaml from example"; fi
	@echo "Please edit config.yaml with your settings"
	@echo "Don't forget to:"
	@echo "  1. Set your Telegram bot token"
	@echo "  2. Configure database URL"
	@echo "  3. Set balancer service URL"

migration-create: ## Create a new database migration
	@if [ -z "$(name)" ]; then echo "Usage: make migration-create name=<migration_name>"; exit 1; fi
	sqlx migrate add $(name)

migration-run: ## Run database migrations
	sqlx migrate run

migration-revert: ## Revert last database migration
	sqlx migrate revert

check: ## Check code with clippy
	cargo clippy -- -D warnings

fmt: ## Format code
	cargo fmt

fmt-check: ## Check code formatting
	cargo fmt --check

watch: ## Watch for changes and rebuild
	cargo watch -x 'run -- start'

generate-wallet: ## Generate a new wallet (utility command)
	cargo run -- generate-wallet

version: ## Show version information
	cargo run -- version

# Docker commands
docker-build: ## Build Docker image
	docker build -t ledgerflow-bot .

docker-run: ## Run Docker container
	docker run --rm -it \
		-v $(PWD)/config.yaml:/etc/ledgerflow/config.yaml \
		-e RUST_LOG=info \
		ledgerflow-bot

# Development utilities
dev-setup: ## Setup development database
	@echo "Setting up development database..."
	@read -p "Enter PostgreSQL username: " username; \
	read -p "Enter database name (default: ledgerflow): " dbname; \
	dbname=$${dbname:-ledgerflow}; \
	createdb -U $$username $$dbname || echo "Database might already exist"; \
	echo "Database setup complete"

dev-db-reset: ## Reset development database
	@echo "Resetting development database..."
	@read -p "Enter PostgreSQL username: " username; \
	read -p "Enter database name (default: ledgerflow): " dbname; \
	dbname=$${dbname:-ledgerflow}; \
	dropdb -U $$username $$dbname --if-exists; \
	createdb -U $$username $$dbname; \
	make migration-run; \
	echo "Database reset complete"

logs: ## Show application logs (when running in background)
	tail -f /tmp/ledgerflow-bot.log

# CI/CD helpers
ci-test: ## Run tests in CI environment
	cargo test --release

ci-build: ## Build for CI
	cargo build --release --locked

install: ## Install the binary
	cargo install --path .

uninstall: ## Uninstall the binary
	cargo uninstall ledgerflow-bot

# Documentation
docs: ## Generate documentation
	cargo doc --open

docs-deps: ## Install documentation dependencies
	cargo install mdbook

serve-docs: ## Serve documentation locally
	mdbook serve docs/

# Security and audit
audit: ## Run security audit
	cargo audit

update-deps: ## Update dependencies
	cargo update

# Example usage
example-setup: ## Show example setup commands
	@echo "Example setup commands:"
	@echo ""
	@echo "1. Install dependencies:"
	@echo "   make build"
	@echo ""
	@echo "2. Setup configuration:"
	@echo "   make setup"
	@echo ""
	@echo "3. Create database:"
	@echo "   make dev-setup"
	@echo ""
	@echo "4. Run migrations:"
	@echo "   make migration-run"
	@echo ""
	@echo "5. Start the bot:"
	@echo "   make run"
	@echo ""
	@echo "For production:"
	@echo "   make build-release"
	@echo "   make run-release"
