# LedgerFlow Sui Indexer

Real-time event indexer for LedgerFlow Sui payment vault contracts. This indexer monitors the Sui blockchain for events emitted by payment vault smart contracts and stores them in a PostgreSQL database for efficient querying and analysis.

## Overview

The LedgerFlow Sui Indexer is a robust, production-ready service that:

- **Real-time Event Processing**: Monitors Sui blockchain checkpoints for payment vault events
- **Reliable Storage**: Stores events in PostgreSQL with proper indexing for fast queries
- **Fault Tolerance**: Implements retry logic and checkpoint tracking for resilient operation
- **Health Monitoring**: Provides health check endpoints for monitoring and deployment
- **Event Deduplication**: Ensures no duplicate events are stored in the database

## Features

### Event Types Monitored
- **Deposit Events**: Track USDC deposits with order IDs and payer information
- **Withdrawal Events**: Monitor owner withdrawals with recipient details
- **Ownership Transfer Events**: Record vault ownership changes

### Key Capabilities
- **Checkpoint-based Processing**: Processes Sui blockchain checkpoints sequentially
- **Batch Processing**: Configurable batch sizes for optimal performance
- **State Persistence**: Tracks indexing progress to resume after interruptions
- **Event Filtering**: Only processes events from configured payment vault packages
- **Concurrent Safe**: Designed for single-instance deployment with database locks

## Installation

### Prerequisites

- Rust 1.70+ with Cargo
- PostgreSQL 12+ database
- Access to Sui blockchain (devnet, testnet, or mainnet)

### Build from Source

```bash
# From the project root
cargo build --release -p ledgerflow-indexer-sui

# Or from the ledgerflow-indexer-sui directory
cd ledgerflow-indexer-sui
cargo build --release
```

## Configuration

### Initial Setup

1. Create a configuration file:
```bash
cp config.yaml.example config.yaml
```

2. Edit the configuration file with your settings:
```yaml
health_check_port: 8086

network:
  rpc_url: "https://fullnode.devnet.sui.io:443"
  ws_url: "wss://fullnode.devnet.sui.io:443"
  network: "devnet"

contract:
  package_id: "0xabc123...your_package_id"
  module_name: "payment_vault"
  deposit_event_type: "DepositReceived"
  withdraw_event_type: "WithdrawCompleted"
  ownership_transfer_event_type: "OwnershipTransferred"

indexer:
  starting_checkpoint: 0
  checkpoint_batch_size: 100
  processing_delay_ms: 1000
  max_retries: 5
  retry_delay_ms: 5000

database:
  connection_string: "postgresql://postgres:password@localhost:5432/ledgerflow"
  max_connections: 10
  connection_timeout_secs: 30
```

### Configuration Options

#### Network Settings
- `rpc_url`: Sui Full node RPC endpoint
- `ws_url`: WebSocket URL for real-time subscriptions (optional)
- `network`: Network identifier (devnet, testnet, mainnet, localnet)

#### Contract Settings
- `package_id`: Address of the deployed payment vault package
- `module_name`: Module name within the package (typically "payment_vault")
- `deposit_event_type`: Event struct name for deposits
- `withdraw_event_type`: Event struct name for withdrawals
- `ownership_transfer_event_type`: Event struct name for ownership transfers

#### Indexer Behavior
- `starting_checkpoint`: Checkpoint to start indexing from (0 for beginning)
- `checkpoint_batch_size`: Number of checkpoints processed per batch
- `processing_delay_ms`: Delay between processing batches
- `max_retries`: Maximum retry attempts for failed operations
- `retry_delay_ms`: Base delay for exponential backoff

#### Database Settings
- `connection_string`: PostgreSQL connection string
- `max_connections`: Maximum database connection pool size
- `connection_timeout_secs`: Database connection timeout

## Database Setup

### PostgreSQL Database

1. Create a database for the indexer:
```sql
CREATE DATABASE ledgerflow;
```

2. The indexer will automatically run migrations on startup to create the required tables:
   - `sui_indexer_state`: Tracks indexing progress
   - `sui_deposit_events`: Stores deposit event data
   - `sui_withdraw_events`: Stores withdrawal event data
   - `sui_ownership_transfer_events`: Stores ownership transfer event data

### Database Schema

#### Deposit Events Table
```sql
CREATE TABLE sui_deposit_events (
    id SERIAL PRIMARY KEY,
    chain_id VARCHAR(64) NOT NULL,
    package_id VARCHAR(64) NOT NULL,
    vault_id VARCHAR(64) NOT NULL,
    payer VARCHAR(64) NOT NULL,
    order_id VARCHAR(255) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    timestamp BIGINT NOT NULL,
    deposit_index BIGINT NOT NULL,
    checkpoint_sequence BIGINT NOT NULL,
    transaction_digest VARCHAR(255) NOT NULL,
    event_index INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

## Usage

### Running the Indexer

```bash
# Use default configuration file (config.yaml)
ledgerflow-indexer-sui

# Use a specific configuration file
ledgerflow-indexer-sui --config ./my-config.yaml
```

### Environment Variables

Configuration can be overridden using environment variables:

```bash
export RUST_LOG=debug  # Enable debug logging
export LEDGERFLOW_NETWORK_RPC_URL="https://fullnode.testnet.sui.io:443"
export LEDGERFLOW_DATABASE_CONNECTION_STRING="postgresql://user:pass@localhost:5432/db"

ledgerflow-indexer-sui
```

### Health Monitoring

The indexer provides health check endpoints:

```bash
# Basic health check
curl http://localhost:8086/health

# Readiness check (includes database connectivity)
curl http://localhost:8086/ready
```

Example health response:
```json
{
  "status": "healthy",
  "service": "ledgerflow-indexer-sui",
  "timestamp": "2024-08-04T10:30:00Z"
}
```

## Monitoring and Operations

### Logging

The indexer uses structured logging with multiple levels:

```bash
# Set log level
export RUST_LOG=info              # Default: info level
export RUST_LOG=debug             # Debug: detailed processing information
export RUST_LOG=ledgerflow_indexer_sui=trace  # Trace: maximum verbosity
```

### Key Metrics to Monitor

- **Checkpoint Progress**: Monitor `last_processed_checkpoint` in database
- **Event Processing Rate**: Track events inserted per minute
- **Database Connection Health**: Monitor `/ready` endpoint
- **Error Rates**: Watch for consecutive processing failures

### Operational Considerations

#### Scaling
- **Single Instance**: Run only one indexer instance per package to avoid conflicts
- **Database Performance**: Monitor database CPU/memory under load
- **Checkpoint Lag**: Track difference between latest and processed checkpoints

#### Recovery
- **Automatic Resume**: Indexer automatically resumes from last processed checkpoint
- **Manual Reset**: Modify `sui_indexer_state` table to restart from specific checkpoint
- **Event Replay**: Safe to restart - duplicate events are automatically handled

## Querying Indexed Data

### Example Queries

#### Get Recent Deposits for a Vault
```sql
SELECT 
    payer, 
    order_id, 
    amount::text as amount_str,
    timestamp,
    transaction_digest
FROM sui_deposit_events 
WHERE vault_id = '0x123abc...' 
    AND chain_id = 'devnet'
ORDER BY checkpoint_sequence DESC 
LIMIT 10;
```

#### Get All Events for a Specific Transaction
```sql
-- Deposits
SELECT 'deposit' as event_type, payer as participant, amount::text
FROM sui_deposit_events 
WHERE transaction_digest = '0xabcdef...'

UNION ALL

-- Withdrawals  
SELECT 'withdraw' as event_type, recipient as participant, amount::text
FROM sui_withdraw_events 
WHERE transaction_digest = '0xabcdef...'

ORDER BY event_type;
```

#### Track Vault Activity Over Time
```sql
SELECT 
    DATE_TRUNC('hour', to_timestamp(timestamp / 1000)) as hour,
    COUNT(*) as deposit_count,
    SUM(amount) as total_volume
FROM sui_deposit_events
WHERE vault_id = '0x123abc...'
    AND timestamp >= EXTRACT(epoch FROM NOW() - INTERVAL '24 hours') * 1000
GROUP BY hour
ORDER BY hour;
```

## Development

### Building
```bash
cargo build
```

### Testing
```bash
cargo test
```

### Linting
```bash
cargo clippy
```

### Formatting
```bash
cargo fmt
```

### Running with Debug Logging
```bash
RUST_LOG=debug cargo run -- --config config.yaml
```

## Troubleshooting

### Common Issues

#### "Failed to connect to Sui network"
- Verify RPC URL is correct and accessible
- Check network connectivity
- Ensure the Sui node is operational and not rate-limiting

#### "Database connection failed"
- Verify PostgreSQL is running and accessible
- Check connection string format and credentials
- Ensure database exists and user has appropriate permissions

#### "No events found for package"
- Verify package ID is correct and deployed on target network
- Check that contract events are being emitted
- Ensure event type names match contract implementation

#### "Checkpoint processing falling behind"
- Increase `checkpoint_batch_size` for faster processing
- Monitor database performance and connection pool usage
- Consider scaling database resources

### Debug Mode

Enable verbose logging for detailed troubleshooting:
```bash
RUST_LOG=ledgerflow_indexer_sui=debug ledgerflow-indexer-sui --config config.yaml
```

## Integration Examples

### Monitoring Script
```bash
#!/bin/bash
# Check indexer health and checkpoint progress

HEALTH_URL="http://localhost:8086/ready"
DB_QUERY="SELECT last_processed_checkpoint FROM sui_indexer_state WHERE package_id = '$PACKAGE_ID'"

# Check health
if curl -f "$HEALTH_URL" > /dev/null 2>&1; then
    echo "✅ Indexer is healthy"
else
    echo "❌ Indexer health check failed"
    exit 1
fi

# Check checkpoint progress
LAST_CHECKPOINT=$(psql "$DATABASE_URL" -t -c "$DB_QUERY" | xargs)
echo "📍 Last processed checkpoint: $LAST_CHECKPOINT"
```

### Event Processing Script
```bash
#!/bin/bash
# Process new deposit events

NEW_DEPOSITS=$(psql "$DATABASE_URL" -t -c "
    SELECT COUNT(*) FROM sui_deposit_events 
    WHERE created_at > NOW() - INTERVAL '1 hour'
")

echo "📈 New deposits in last hour: $NEW_DEPOSITS"

# Process each new deposit
psql "$DATABASE_URL" -c "
    SELECT order_id, payer, amount::text
    FROM sui_deposit_events 
    WHERE created_at > NOW() - INTERVAL '1 hour'
    ORDER BY created_at
" | while read order_id payer amount; do
    echo "💰 Processing deposit: $order_id from $payer for $amount"
    # Add your business logic here
done
```

## Security Considerations

### Database Security
- Use strong passwords and limit database access
- Enable SSL/TLS for database connections in production
- Regularly backup indexed data
- Monitor for unusual database activity

### Network Security
- Use HTTPS/WSS endpoints only
- Verify RPC endpoint authenticity
- Monitor for unexpected network patterns
- Consider rate limiting for health endpoints

### Operational Security
- Store sensitive configuration in environment variables
- Use proper secrets management in production
- Monitor indexer logs for security events
- Keep dependencies updated

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Ensure all tests pass
5. Submit a pull request

## License

This project is licensed under either of:
- Apache License, Version 2.0
- MIT License

at your option.

## Support

For support and questions:
- GitHub Issues: https://github.com/longcipher/ledgerflow/issues
- Documentation: https://github.com/longcipher/ledgerflow/docs
