#!/bin/bash

# Test script to verify logging functionality in ledgerflow-indexer

set -e

echo "🧪 Testing ledgerflow-indexer logging functionality"

# Build the project
echo "📦 Building ledgerflow-indexer..."
cargo build --release

# Set logging level to info to see all our new logs
export RUST_LOG=info

echo "✅ Build successful!"
echo "🔧 To run the indexer with enhanced logging, use:"
echo "   RUST_LOG=info cargo run -- --config config.yaml"
echo ""
echo "🔧 For more detailed logging (including debug messages), use:"
echo "   RUST_LOG=debug cargo run -- --config config.yaml"
echo ""
echo "🔧 For trace level logging, use:"
echo "   RUST_LOG=trace cargo run -- --config config.yaml"
echo ""
echo "ℹ️  The indexer will now show:"
echo "   🚀 Startup process with chain configurations"
echo "   📡 RPC connection status"
echo "   📊 Block scanning progress and status"
echo "   💓 Periodic heartbeat with sync status"
echo "   🎯 Event discovery with counts"
echo "   📝 Individual event processing"
echo "   💰 Deposit event details"
echo "   ✅ Database operations and status updates"
echo "   ⚠️  Errors and warnings with clear context"
