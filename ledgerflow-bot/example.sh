#!/bin/bash

# LedgerFlow Bot Example Script
# This script demonstrates how to set up and run the LedgerFlow Bot

set -e

echo "🚀 LedgerFlow Bot Example Setup"
echo "================================"

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Check if PostgreSQL is installed
if ! command -v psql &> /dev/null; then
    echo "❌ PostgreSQL is not installed. Please install PostgreSQL first:"
    echo "   macOS: brew install postgresql"
    echo "   Ubuntu: sudo apt-get install postgresql postgresql-contrib"
    exit 1
fi

echo "✅ Prerequisites check passed"

# Build the project
echo "🔨 Building the project..."
cargo build

# Setup configuration
if [ ! -f config.yaml ]; then
    cp config.yaml.example config.yaml
    echo "📋 Created config.yaml from example"
    echo "⚠️  Please edit config.yaml with your settings:"
    echo "   - Add your Telegram bot token"
    echo "   - Update database URL if needed"
    echo "   - Set balancer service URL"
    echo ""
    read -p "Press Enter to continue after editing config.yaml..."
fi

# Database setup
echo "🗄️  Setting up database..."
read -p "Enter PostgreSQL username (default: $USER): " db_user
db_user=${db_user:-$USER}

read -p "Enter database name (default: ledgerflow): " db_name
db_name=${db_name:-ledgerflow}

# Create database if it doesn't exist
createdb -U $db_user $db_name 2>/dev/null || echo "Database $db_name already exists"

# Set DATABASE_URL for migrations
export DATABASE_URL="postgresql://$db_user@localhost:5432/$db_name"

# Install sqlx-cli if not present
if ! command -v sqlx &> /dev/null; then
    echo "📦 Installing sqlx-cli..."
    cargo install sqlx-cli --no-default-features --features postgres
fi

# Run migrations
echo "🔄 Running database migrations..."
sqlx migrate run

echo "✅ Database setup complete"

# Generate example wallet
echo "👛 Generating example wallet..."
cargo run -- generate-wallet

echo ""
echo "🎉 Setup complete! You can now:"
echo "   1. Edit config.yaml with your bot token"
echo "   2. Start the bot with: cargo run"
echo "   3. Or use custom config: cargo run -- --config path/to/custom-config.yaml"
echo "   4. Or use: make run"
echo ""
echo "📚 For more commands, run: make help"
echo ""
echo "🤖 Don't forget to:"
echo "   - Get a bot token from @BotFather on Telegram"
echo "   - Start the LedgerFlow Balancer service"
echo "   - Update the payment vault address in config.yaml"
