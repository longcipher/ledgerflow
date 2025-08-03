#!/bin/bash

# Example script for running the LedgerFlow Sui Indexer

set -e

# Configuration
CONFIG_FILE="config.yaml"
LOG_LEVEL="info"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 Starting LedgerFlow Sui Indexer${NC}"

# Check if config file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${YELLOW}⚠️  Configuration file not found. Creating from example...${NC}"
    if [ -f "config.yaml.example" ]; then
        cp config.yaml.example "$CONFIG_FILE"
        echo -e "${YELLOW}📝 Please edit $CONFIG_FILE with your settings before running${NC}"
        exit 1
    else
        echo -e "${RED}❌ No configuration example found${NC}"
        exit 1
    fi
fi

# Set environment variables
export RUST_LOG=${RUST_LOG:-$LOG_LEVEL}

# Build the indexer
echo -e "${YELLOW}🔨 Building indexer...${NC}"
cargo build --release -p ledgerflow-indexer-sui

# Run the indexer
echo -e "${GREEN}🎯 Starting indexer with config: $CONFIG_FILE${NC}"
exec ./target/release/ledgerflow-indexer-sui --config "$CONFIG_FILE"
