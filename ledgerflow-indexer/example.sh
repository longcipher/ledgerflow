#!/bin/bash

# LedgerFlow Indexer Example Usage Script

set -e

echo "🚀 LedgerFlow Indexer Example Usage"
echo "====================================="

# Configuration
CONFIG_FILE="config.yaml"
DATABASE_URL="postgres://ledgerflow:ledgerflow123@localhost:5432/ledgerflow"

# Check if config exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo "📋 Creating configuration file..."
    cat > "$CONFIG_FILE" << EOF
chains:
  - name: "sepolia"
    rpc_http: "https://sepolia.unichain.org"
    rpc_ws: "wss://sepolia.unichain.org/ws"
    payment_vault_contract: "0x742d35Cc6634C0532925a3b8D11C5d2B7e5B3F6E"
    start_block: 0

database:
  url: "$DATABASE_URL"
EOF
    echo "✅ Created $CONFIG_FILE"
fi

# Check if binary exists
if [ ! -f "./target/release/ledgerflow-indexer" ]; then
    echo "🔨 Building indexer..."
    cargo build --release
    echo "✅ Build completed"
fi

# Function to check database connection
check_database() {
    echo "🔍 Checking database connection..."
    if command -v psql &> /dev/null; then
        if psql "$DATABASE_URL" -c "SELECT 1;" &> /dev/null; then
            echo "✅ Database connection successful"
            return 0
        else
            echo "❌ Database connection failed"
            return 1
        fi
    else
        echo "⚠️  psql not found, skipping database check"
        return 0
    fi
}

# Function to show database status
show_database_status() {
    if command -v psql &> /dev/null && psql "$DATABASE_URL" -c "SELECT 1;" &> /dev/null; then
        echo ""
        echo "📊 Database Status:"
        echo "==================="
        
        # Chain states
        echo "📈 Chain States:"
        psql "$DATABASE_URL" -c "
            SELECT 
                chain_name,
                contract_address,
                last_scanned_block,
                updated_at
            FROM chain_states 
            ORDER BY chain_name;
        " 2>/dev/null || echo "  No chain states found"
        
        # Recent events
        echo ""
        echo "🎯 Recent Events (last 10):"
        psql "$DATABASE_URL" -c "
            SELECT 
                chain_name,
                LEFT(order_id, 16) || '...' as order_id,
                LEFT(sender, 10) || '...' as sender,
                amount,
                block_number,
                processed
            FROM deposit_events 
            ORDER BY created_at DESC 
            LIMIT 10;
        " 2>/dev/null || echo "  No events found"
        
        # Summary stats
        echo ""
        echo "📈 Summary Statistics:"
        psql "$DATABASE_URL" -c "
            SELECT 
                chain_name,
                COUNT(*) as total_events,
                COUNT(*) FILTER (WHERE processed = true) as processed_events,
                COUNT(*) FILTER (WHERE processed = false) as pending_events,
                MAX(block_number) as latest_block
            FROM deposit_events 
            GROUP BY chain_name
            ORDER BY chain_name;
        " 2>/dev/null || echo "  No statistics available"
    fi
}

# Main execution
main() {
    echo ""
    echo "🏁 Starting LedgerFlow Indexer Example"
    echo ""
    
    # Check database
    if ! check_database; then
        echo ""
        echo "ℹ️  To set up PostgreSQL:"
        echo "   1. Install PostgreSQL"
        echo "   2. Create database: createdb ledgerflow"
        echo "   3. Create user: psql -c \"CREATE USER ledgerflow WITH PASSWORD 'ledgerflow123';\""
        echo "   4. Grant permissions: psql -c \"GRANT ALL PRIVILEGES ON DATABASE ledgerflow TO ledgerflow;\""
        echo ""
        echo "🚀 Running indexer anyway (will show connection errors)..."
    fi
    
    echo ""
    echo "🎯 Configuration:"
    echo "   Config file: $CONFIG_FILE"
    echo "   Database: $DATABASE_URL"
    echo ""
    
    # Show current database status
    show_database_status
    
    echo ""
    echo "🚀 Starting indexer..."
    echo "   Press Ctrl+C to stop"
    echo ""
    
    # Run the indexer with info logging
    RUST_LOG=info ./target/release/ledgerflow-indexer --config "$CONFIG_FILE"
}

# Handle cleanup
cleanup() {
    echo ""
    echo "🛑 Stopping indexer..."
    echo ""
    echo "📊 Final database status:"
    show_database_status
    echo ""
    echo "✅ Example completed"
}

# Set up signal handling
trap cleanup EXIT INT TERM

# Run main function
main
