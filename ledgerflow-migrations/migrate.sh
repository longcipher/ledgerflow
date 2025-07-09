#!/bin/bash

# LedgerFlow Database Migration Script
# This script uses sqlx-cli to manage database migrations

set -e

# Default values
DATABASE_URL="${DATABASE_URL:-postgresql://postgres:password@localhost/ledgerflow}"
COMMAND="${1:-migrate}"

echo "Using DATABASE_URL: $DATABASE_URL"

case $COMMAND in
    "migrate"|"run")
        echo "Running database migrations..."
        sqlx migrate run --database-url "$DATABASE_URL"
        ;;
    "revert")
        STEPS="${2:-1}"
        echo "Reverting $STEPS migration step(s)..."
        sqlx migrate revert --database-url "$DATABASE_URL" --steps "$STEPS"
        ;;
    "add")
        if [ -z "$2" ]; then
            echo "Usage: $0 add <migration_name>"
            exit 1
        fi
        echo "Adding new migration: $2"
        sqlx migrate add "$2" --database-url "$DATABASE_URL"
        ;;
    "info")
        echo "Migration info:"
        sqlx migrate info --database-url "$DATABASE_URL"
        ;;
    "reset")
        echo "Resetting database and re-running all migrations..."
        sqlx database reset --database-url "$DATABASE_URL"
        ;;
    "setup")
        echo "Setting up database..."
        sqlx database create --database-url "$DATABASE_URL"
        sqlx migrate run --database-url "$DATABASE_URL"
        ;;
    *)
        echo "Usage: $0 [migrate|revert|add|info|reset|setup] [args...]"
        echo ""
        echo "Commands:"
        echo "  migrate, run    - Run pending migrations"
        echo "  revert [steps]  - Revert migrations (default: 1 step)"
        echo "  add <name>      - Add new migration file"
        echo "  info            - Show migration status"
        echo "  reset           - Reset database and re-run all migrations"
        echo "  setup           - Create database and run migrations"
        echo ""
        echo "Environment variables:"
        echo "  DATABASE_URL    - Database connection string"
        exit 1
        ;;
esac

echo "Migration command completed successfully!"
