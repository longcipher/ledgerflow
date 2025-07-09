# LedgerFlow Indexer - Enhanced Logging

## Overview

The LedgerFlow Indexer has been enhanced with comprehensive logging to provide clear visibility into the program's execution stages. The logging uses structured messages with emojis for better readability and includes different log levels for various use cases.

## Log Levels

The indexer uses standard Rust logging levels:

- **ERROR**: Critical errors that prevent normal operation
- **WARN**: Warnings about potential issues or unexpected conditions
- **INFO**: General information about program execution (default level)
- **DEBUG**: Detailed information for debugging purposes
- **TRACE**: Very detailed tracing information

## Setting Log Levels

Use the `RUST_LOG` environment variable to control logging verbosity:

```bash
# Info level (recommended for normal operation)
RUST_LOG=info cargo run -- --config config.yaml

# Debug level (for troubleshooting)
RUST_LOG=debug cargo run -- --config config.yaml

# Trace level (for detailed debugging)
RUST_LOG=trace cargo run -- --config config.yaml
```

## Logging Features

### 1. Startup Process
- 🚀 **Program initialization** - Shows startup message and configuration file being used
- 📋 **Configuration loading** - Displays number of chains and their details
- ⛓️  **Chain information** - Shows each chain's ID, name, RPC endpoint, and contract address
- ✅ **Database connection** - Confirms successful database connection
- 🔥 **Indexer start** - Indicates when indexing process begins

### 2. Chain Indexing Process
- 🚀 **Chain startup** - Shows when indexing starts for each chain
- 📡 **RPC connection** - Confirms connection to blockchain RPC endpoints
- 📊 **Block scanning status** - Shows starting block and contract being monitored
- 🔍 **Event monitoring** - Indicates what events are being listened for

### 3. Block Processing
- 💓 **Heartbeat messages** - Periodic status updates every ~60 seconds showing:
  - Current blockchain block number
  - Last scanned block
  - Number of blocks behind
- ⏭️ **Catch-up progress** - Shows when catching up with missed blocks
- 📦 **Batch processing** - Progress updates for block batch processing

### 4. Event Processing
- 🎯 **Event discovery** - Shows number of events found in each block range
- 📝 **Individual event processing** - Progress for each event being processed
- 🔍 **Event parsing** - Details about parsing deposit events
- 💰 **Deposit details** - Shows order ID, sender address, and amount

### 5. Database Operations
- **Connection status** - Database connection success/failure
- **Query execution** - Info about database queries being executed
- **Data updates** - Confirmation of successful data insertions and updates
- **Chain state tracking** - Updates to last scanned block positions

### 6. Error Handling
- ❌ **RPC errors** - Network connectivity issues with blockchain nodes
- ❌ **Database errors** - Database connection or query failures
- ❌ **Parsing errors** - Issues with event data parsing
- ⚠️ **Warnings** - Duplicate events or missing orders

## Example Log Output

```
2024-01-15T10:30:00.123Z  INFO ledgerflow_indexer: 🚀 Starting LedgerFlow Indexer
2024-01-15T10:30:00.125Z  INFO ledgerflow_indexer: 📋 Configuration file: config.yaml
2024-01-15T10:30:00.130Z  INFO ledgerflow_indexer: ✅ Loaded configuration for 2 chains
2024-01-15T10:30:00.131Z  INFO ledgerflow_indexer: ⛓️  Chain: Ethereum (ID: 1) - RPC: https://eth-mainnet.g.alchemy.com/v2/...
2024-01-15T10:30:00.131Z  INFO ledgerflow_indexer: ⛓️  Chain: Polygon (ID: 137) - RPC: https://polygon-mainnet.g.alchemy.com/v2/...
2024-01-15T10:30:00.200Z  INFO ledgerflow_indexer::database: Connecting to database: postgresql://...
2024-01-15T10:30:00.250Z  INFO ledgerflow_indexer::database: Successfully connected to database
2024-01-15T10:30:00.251Z  INFO ledgerflow_indexer: ✅ Connected to database successfully
2024-01-15T10:30:00.252Z  INFO ledgerflow_indexer: ✅ Indexer initialized successfully
2024-01-15T10:30:00.253Z  INFO ledgerflow_indexer: 🔥 Starting indexing process...
2024-01-15T10:30:00.254Z  INFO ledgerflow_indexer::indexer: Starting indexer for 2 chains
2024-01-15T10:30:00.255Z  INFO ledgerflow_indexer::indexer: Spawning indexer task for chain: Ethereum (chain_id: 1)
2024-01-15T10:30:00.256Z  INFO ledgerflow_indexer::indexer: Spawning indexer task for chain: Polygon (chain_id: 137)
2024-01-15T10:30:00.257Z  INFO ledgerflow_indexer::indexer: All indexer tasks spawned, waiting for completion...
2024-01-15T10:30:00.258Z  INFO ledgerflow_indexer::indexer: 🚀 Starting indexer for chain: Ethereum (chain_id: 1)
2024-01-15T10:30:00.259Z  INFO ledgerflow_indexer::indexer: 📡 Connecting to RPC endpoint: https://eth-mainnet.g.alchemy.com/v2/...
2024-01-15T10:30:00.350Z  INFO ledgerflow_indexer::indexer: ✅ Connected to RPC for chain: Ethereum
2024-01-15T10:30:00.351Z  INFO ledgerflow_indexer::database: Getting chain state for chain_id: 1, contract: 0x1234...
2024-01-15T10:30:00.365Z  INFO ledgerflow_indexer::database: Found chain state for chain 1: last_scanned_block = 18500000
2024-01-15T10:30:00.366Z  INFO ledgerflow_indexer::indexer: 📊 Starting from block 18500000 for chain Ethereum (contract: 0x1234...)
2024-01-15T10:30:00.367Z  INFO ledgerflow_indexer::indexer: 🔍 Listening for DepositReceived events on contract: 0x1234...
2024-01-15T10:30:00.368Z  INFO ledgerflow_indexer::indexer: 💓 Chain Ethereum heartbeat - Current block: 18500150, Last scanned: 18500000, Blocks behind: 150
2024-01-15T10:30:00.369Z  INFO ledgerflow_indexer::indexer: ⏭️ Chain Ethereum catching up: 150 blocks behind (from 18500000 to 18500150)
2024-01-15T10:30:00.370Z  INFO ledgerflow_indexer::indexer: 📦 Processing batch 18500001-18500100 for chain Ethereum (100/150)
2024-01-15T10:30:01.500Z  INFO ledgerflow_indexer::indexer: 🎯 Found 2 DepositReceived events in blocks 18500001-18500100 for chain Ethereum
2024-01-15T10:30:01.501Z  INFO ledgerflow_indexer::indexer: 📝 Processing event 1/2 in block 18500050 for chain Ethereum
2024-01-15T10:30:01.502Z  INFO ledgerflow_indexer::indexer: 🔍 Parsing deposit event from tx: 0xabcd... (block: 18500050, log_index: 0)
2024-01-15T10:30:01.503Z  INFO ledgerflow_indexer::indexer: 💰 Deposit event parsed - Order: 1234567890abcdef, Sender: 0x5678..., Amount: 1000000000000000000
2024-01-15T10:30:01.504Z  INFO ledgerflow_indexer::database: Inserting deposit event: order_id=1234567890abcdef, chain_id=1, block=18500050, tx_hash=0xabcd...
2024-01-15T10:30:01.520Z  INFO ledgerflow_indexer::database: Inserted new deposit event for order_id: 1234567890abcdef
2024-01-15T10:30:01.521Z  INFO ledgerflow_indexer::database: Updating order status to 'deposited' for order_id: 1234567890abcdef, tx_hash: 0xabcd...
2024-01-15T10:30:01.535Z  INFO ledgerflow_indexer::database: Updated order 1234567890abcdef status to 'deposited'
2024-01-15T10:30:01.536Z  INFO ledgerflow_indexer::indexer: ✅ Successfully processed deposit event for order 1234567890abcdef on chain Ethereum - Status updated to 'deposited'
```

## Troubleshooting

If you don't see expected log output:

1. **Check log level**: Make sure `RUST_LOG` is set to `info` or lower
2. **Verify configuration**: Ensure your `config.yaml` file is properly configured
3. **Check database connection**: Ensure your database is running and accessible
4. **Verify RPC endpoints**: Make sure your blockchain RPC endpoints are working

## Performance Considerations

- **INFO level**: Recommended for production use, provides good visibility without excessive output
- **DEBUG level**: Use for troubleshooting, generates more output
- **TRACE level**: Only use for detailed debugging, generates very verbose output

The logging is designed to be helpful without significantly impacting performance, but higher log levels will generate more output and slightly increase processing time.
