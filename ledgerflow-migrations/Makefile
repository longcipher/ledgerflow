# LedgerFlow Migrations Makefile

.PHONY: help setup migrate revert add info reset clean install

# Default database URL
DATABASE_URL ?= postgresql://postgres:password@localhost/ledgerflow

help:
	@echo "LedgerFlow Migration Commands:"
	@echo "  make install    - Install sqlx-cli"
	@echo "  make setup      - Create database and run migrations"
	@echo "  make migrate    - Run pending migrations"
	@echo "  make revert     - Revert last migration"
	@echo "  make add NAME=<migration_name> - Add new migration"
	@echo "  make info       - Show migration status"
	@echo "  make reset      - Reset database and re-run all migrations"
	@echo "  make clean      - Clean build artifacts"
	@echo ""
	@echo "Environment variables:"
	@echo "  DATABASE_URL    - Database connection string (current: $(DATABASE_URL))"

install:
	@echo "Installing sqlx-cli..."
	cargo install sqlx-cli --no-default-features --features postgres

setup:
	@echo "Setting up database..."
	DATABASE_URL=$(DATABASE_URL) ./migrate.sh setup

migrate:
	@echo "Running migrations..."
	DATABASE_URL=$(DATABASE_URL) ./migrate.sh migrate

revert:
	@echo "Reverting migrations..."
	DATABASE_URL=$(DATABASE_URL) ./migrate.sh revert

add:
	@if [ -z "$(NAME)" ]; then \
		echo "Usage: make add NAME=<migration_name>"; \
		exit 1; \
	fi
	@echo "Adding migration: $(NAME)"
	DATABASE_URL=$(DATABASE_URL) ./migrate.sh add "$(NAME)"

info:
	@echo "Migration info:"
	DATABASE_URL=$(DATABASE_URL) ./migrate.sh info

reset:
	@echo "Resetting database..."
	DATABASE_URL=$(DATABASE_URL) ./migrate.sh reset

clean:
	@echo "Cleaning build artifacts..."
	cargo clean

# Build the migration manager
build:
	@echo "Building migration manager..."
	cargo build --release

# Run migration manager binary
run:
	@echo "Running migration manager..."
	DATABASE_URL=$(DATABASE_URL) cargo run --release
