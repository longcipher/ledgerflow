#!/bin/bash

# LedgerFlow Indexer Setup Script

echo "Setting up LedgerFlow Indexer..."

# Check if PostgreSQL is installed
if ! command -v psql &> /dev/null; then
    echo "PostgreSQL is not installed. Please install PostgreSQL first."
    exit 1
fi

# Create database if it doesn't exist
DB_NAME="ledgerflow"
DB_USER="ledgerflow"
DB_PASSWORD="ledgerflow123"

echo "Creating database and user..."
sudo -u postgres psql -c "CREATE USER $DB_USER WITH PASSWORD '$DB_PASSWORD';"
sudo -u postgres psql -c "CREATE DATABASE $DB_NAME OWNER $DB_USER;"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE $DB_NAME TO $DB_USER;"

echo "Database setup complete!"

# Create config file from example
if [ ! -f "config.yaml" ]; then
    cp config.example.yaml config.yaml
    echo "Created config.yaml from example. Please update it with your settings."
fi

# Build the project
echo "Building the project..."
cargo build --release

echo "Setup complete!"
echo "Update config.yaml with your chain configurations and database URL."
echo "Run with: cargo run --release -- --config config.yaml"
