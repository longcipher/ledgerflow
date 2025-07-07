.PHONY: build run test clean setup fmt check migrate

# Default target
all: build

# Build the project
build:
	cargo build --release

# Run the indexer
run:
	cargo run --release -- --config config.yaml

# Run tests
test:
	cargo test

# Clean build artifacts
clean:
	cargo clean

# Setup development environment
setup:
	./setup.sh

# Format code
fmt:
	cargo fmt

# Check code with clippy
check:
	cargo clippy -- -D warnings

# Run database migrations manually
migrate:
	sqlx migrate run --database-url "postgres://ledgerflow:ledgerflow123@localhost:5432/ledgerflow"

# Development run with logging
dev:
	RUST_LOG=info cargo run -- --config config.yaml

# Install sqlx-cli if needed
install-sqlx:
	cargo install sqlx-cli --no-default-features --features postgres
