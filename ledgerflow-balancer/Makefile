# LedgerFlow Balancer Makefile

.PHONY: build run test clean setup migrate dev lint

# Build the project
build:
	cargo build --release

# Run the project
run:
	cargo run

# Run tests
test:
	cargo test

# Clean build artifacts
clean:
	cargo clean

# Setup development environment
setup:
	cargo install sqlx-cli
	@echo "Please create a PostgreSQL database and update config.yaml"

# Run database migrations
migrate:
	sqlx migrate run

# Development mode with auto-reload
dev:
	cargo watch -x run

# Lint code
lint:
	cargo clippy -- -D warnings
	cargo fmt --check

# Format code
format:
	cargo fmt

# Check dependencies
check:
	cargo check

# Build for production
build-prod:
	cargo build --release

# Run with specific config
run-config:
	cargo run -- --config config.yaml

# Database commands
db-create:
	createdb ledgerflow

db-drop:
	dropdb ledgerflow

db-reset: db-drop db-create migrate

# Docker commands
docker-build:
	docker build -t ledgerflow-balancer .

docker-run:
	docker run -p 3000:3000 ledgerflow-balancer

# Generate SQL schema
generate-schema:
	sqlx migrate info

# Install dev dependencies
install-dev:
	cargo install cargo-watch
	cargo install sqlx-cli

# Help
help:
	@echo "Available targets:"
	@echo "  build        - Build the project"
	@echo "  run          - Run the project"
	@echo "  test         - Run tests"
	@echo "  clean        - Clean build artifacts"
	@echo "  setup        - Setup development environment"
	@echo "  migrate      - Run database migrations"
	@echo "  dev          - Development mode with auto-reload"
	@echo "  lint         - Lint code"
	@echo "  format       - Format code"
	@echo "  check        - Check dependencies"
	@echo "  db-create    - Create database"
	@echo "  db-drop      - Drop database"
	@echo "  db-reset     - Reset database"
	@echo "  docker-build - Build Docker image"
	@echo "  docker-run   - Run Docker container"
