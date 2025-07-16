# LedgerFlow Indexer

A high-performance Rust-based indexer for real-time monitoring of PaymentVault contract `DepositReceived` events across multiple EVM-compatible chains.

## Features

- **Multi-chain Event Listening**: Monitors `DepositReceived` events from multiple PaymentVault contracts simultaneously
- **Reliable Event Fetching**: Hybrid approach using HTTP RPC (for historical scanning and fallback) and WebSocket RPC (for real-time updates)
- **Event Parsing**: Extracts and parses `orderId`, `sender`, `amount`, `transactionHash`, `blockNumber`, and other event data
- **PostgreSQL Persistence**: Stores parsed event data with deduplication and marks orders as completed
- **Resumable Operations**: Automatically resumes from the last scanned block height for each chain/contract
- **Error Handling**: Robust error handling with automatic retry mechanisms
- **Configurable**: YAML-based configuration for easy deployment and maintenance

## Tech Stack

- **Rust** - High-performance systems programming language
- **[clap](https://clap.rs/)** - Command-line argument parsing
- **[sqlx](https://docs.rs/sqlx/)** - Async PostgreSQL driver with compile-time checked queries
- **[alloy](https://alloy.rs/)** - Modern Ethereum library for Rust
- **[tokio](https://tokio.rs/)** - Async runtime for high-performance I/O

## Installation

### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs/))
- PostgreSQL 12+ (for data persistence)

### Build from Source

```bash
# Clone the repository (if not already done)
git clone <repository-url>
cd ledgerflow-vault/ledgerflow-indexer

# Build the project
cargo build --release

# The binary will be available at ./target/release/ledgerflow-indexer
```

### Quick Setup

```bash
# Run the setup script (requires PostgreSQL)
chmod +x setup.sh
./setup.sh

# Or use make
make setup
```

## Configuration

Create a `config.yaml` file based on the provided example:

```yaml
chains:
  - name: "sepolia"
    rpc_http: "https://sepolia.unichain.org"
    rpc_ws: "wss://sepolia.unichain.org/ws"
    payment_vault_contract: "0x742d35Cc6634C0532925a3b8D11C5d2B7e5B3F6E"
    start_block: 0
  - name: "mainnet"
    rpc_http: "https://mainnet.infura.io/v3/YOUR_PROJECT_ID"
    rpc_ws: "wss://mainnet.infura.io/ws/v3/YOUR_PROJECT_ID"
    payment_vault_contract: "0x..."
    start_block: 18000000

database:
  url: "postgres://user:password@localhost:5432/ledgerflow"
```

### Configuration Options

- **chains**: Array of blockchain configurations
  - **name**: Unique identifier for the chain
  - **rpc_http**: HTTP RPC endpoint URL
  - **rpc_ws**: WebSocket RPC endpoint URL
  - **payment_vault_contract**: PaymentVault contract address
  - **start_block**: Block number to start indexing from (0 for genesis)

- **database**: PostgreSQL connection settings
  - **url**: Full PostgreSQL connection string

## Usage

### Running the Indexer

```bash
# Using default config.yaml
./target/release/ledgerflow-indexer

# Using custom configuration file
./target/release/ledgerflow-indexer --config /path/to/config.yaml

# With logging
RUST_LOG=info ./target/release/ledgerflow-indexer
```

### Database Setup

The indexer automatically creates and manages the required database tables:

- **chain_states**: Tracks the last scanned block for each chain/contract
- **deposit_events**: Stores parsed deposit events with deduplication

### Make Commands

```bash
# Build the project
make build

# Run with default config
make run

# Run in development mode with logging
make dev

# Run tests
make test

# Format code
make fmt

# Check with clippy
make check

# Setup development environment
make setup
```

## Database Schema

### chain_states
```sql
CREATE TABLE chain_states (
    chain_name VARCHAR(255) NOT NULL,
    contract_address VARCHAR(255) NOT NULL,
    last_scanned_block BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    PRIMARY KEY (chain_name, contract_address)
);
```

### deposit_events
```sql
CREATE TABLE deposit_events (
    id BIGSERIAL PRIMARY KEY,
    chain_name VARCHAR(255) NOT NULL,
    contract_address VARCHAR(255) NOT NULL,
    order_id VARCHAR(255) NOT NULL,
    sender VARCHAR(255) NOT NULL,
    amount VARCHAR(255) NOT NULL,
    transaction_hash VARCHAR(255) NOT NULL,
    block_number BIGINT NOT NULL,
    log_index BIGINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed BOOLEAN NOT NULL DEFAULT false,
    UNIQUE (chain_name, transaction_hash, log_index)
);
```

## Architecture

### Event Processing Flow

1. **Initialization**: Load configuration and connect to database
2. **State Recovery**: Read last scanned block for each chain
3. **Historical Sync**: Process missed blocks using HTTP RPC
4. **Real-time Monitoring**: Switch to WebSocket for new events
5. **Event Parsing**: Extract and validate event data
6. **Persistence**: Store events in PostgreSQL with deduplication
7. **State Update**: Update last scanned block

### Error Handling

- **Network Issues**: Automatic retry with exponential backoff
- **RPC Failures**: Fallback from WebSocket to HTTP RPC
- **Database Errors**: Transaction rollback and retry
- **Invalid Events**: Log error and continue processing

## Monitoring

### Logging

Set the `RUST_LOG` environment variable to control log levels:

```bash
# Info level (recommended for production)
RUST_LOG=info ./target/release/ledgerflow-indexer

# Debug level (for troubleshooting)
RUST_LOG=debug ./target/release/ledgerflow-indexer

# Module-specific logging
RUST_LOG=ledgerflow_indexer=debug,sqlx=info ./target/release/ledgerflow-indexer
```

### Key Metrics

Monitor these database queries to track indexer health:

```sql
-- Check processing status
SELECT chain_name, last_scanned_block, updated_at 
FROM chain_states;

-- Count unprocessed events
SELECT chain_name, COUNT(*) 
FROM deposit_events 
WHERE processed = false 
GROUP BY chain_name;

-- Recent events
SELECT * FROM deposit_events 
WHERE created_at > NOW() - INTERVAL '1 hour' 
ORDER BY created_at DESC;
```

## Development

### Testing

```bash
# Run unit tests
cargo test

# Run integration tests (requires test database)
cargo test --features integration-tests

# Test with specific RPC endpoints
./test.sh
```

### Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run `make check` and `make fmt`
6. Submit a pull request

## Production Deployment

### Docker Support

```dockerfile
# Dockerfile example
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/ledgerflow-indexer /usr/local/bin/
CMD ["ledgerflow-indexer"]
```

### Systemd Service

```ini
# /etc/systemd/system/ledgerflow-indexer.service
[Unit]
Description=LedgerFlow Indexer
After=network.target

[Service]
Type=simple
User=ledgerflow
WorkingDirectory=/opt/ledgerflow-indexer
ExecStart=/opt/ledgerflow-indexer/ledgerflow-indexer --config /etc/ledgerflow/config.yaml
Restart=always
RestartSec=10
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

## Security

- Store sensitive configuration (database passwords, RPC URLs) in environment variables
- Use read-only database users when possible
- Implement proper firewall rules for database access
- Monitor for unusual activity patterns

## Performance

- **Batch Size**: Processes up to 100 blocks per batch to optimize RPC calls
- **Concurrent Chains**: Each chain runs in its own async task
- **Database Indexing**: Optimized indexes for common query patterns
- **Memory Usage**: Efficient streaming of large block ranges

## Troubleshooting

### Common Issues

1. **Database Connection Failed**
   ```bash
   # Check PostgreSQL service
   sudo systemctl status postgresql
   
   # Test connection
   psql "postgres://user:password@localhost:5432/ledgerflow"
   ```

2. **RPC Endpoint Issues**
   ```bash
   # Test HTTP endpoint
   curl -X POST -H "Content-Type: application/json" \
     --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
     https://sepolia.unichain.org
   ```

3. **Missing Events**
   - Check `start_block` configuration
   - Verify contract address
   - Review RPC endpoint rate limits

### Debug Mode

```bash
RUST_LOG=debug ./target/release/ledgerflow-indexer --config config.yaml 2>&1 | tee indexer.log
```

## License

This project is licensed under the Apache License 2.0 - see the main project LICENSE file for details.

## Support

For issues and questions:
- Check the troubleshooting section above
- Review logs with debug level enabled
- Open an issue with detailed error information
