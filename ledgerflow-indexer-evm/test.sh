#!/bin/bash

# Test script for LedgerFlow Indexer

echo "Testing LedgerFlow Indexer..."

# Check if config file exists
if [ ! -f "config.yaml" ]; then
    echo "Creating config.yaml from example..."
    cp config.example.yaml config.yaml
fi

# Build the project
echo "Building project..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
else
    echo "❌ Build failed!"
    exit 1
fi

# Test with --help
echo "Testing --help flag..."
./target/release/ledgerflow-indexer --help

echo "Test completed!"
echo "To run the indexer:"
echo "1. Update config.yaml with your settings"
echo "2. Set up PostgreSQL database"
echo "3. Run: ./target/release/ledgerflow-indexer --config config.yaml"
