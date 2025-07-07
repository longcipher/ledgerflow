# LedgerFlow Balancer

LedgerFlow Balancer is the backend service for the LedgerFlow payment system. It serves as the business logic core that connects user frontends with off-chain data.

## Features

- **Account Management**: Associate Email/Telegram ID with EVM addresses
- **Order Creation**: REST API to generate unique order IDs and store them in database
- **Status Queries**: API to query order status by order ID
- **Balance Queries**: API to query account total balance by aggregating completed orders
- **Business Rules**: Limit each account to maximum 2 "pending" orders
- **Admin Interface**: Interface for administrators to view all incomplete orders

## Technology Stack

- **Rust**: Core language
- **Axum**: Web framework
- **SQLx**: Database ORM
- **PostgreSQL**: Database
- **Clap**: CLI argument parsing
- **Config**: Configuration management
- **Tracing**: Logging and observability
- **Eyre**: Error handling

## Architecture

### Order ID Generation Algorithm

The system uses keccak256 hash algorithm to ensure order ID uniqueness, collision resistance, and unpredictability:

```
order_id = keccak256(abi.encodePacked(broker_id, account_id, order_id_num))
```

Where:
- `broker_id`: Unique identifier for merchant/platform
- `account_id`: Unique identifier for paying user (e.g., Telegram user ID)
- `order_id_num`: Order sequence number for the account

### Payment Flow

1. **Merchant Request**: Merchant initiates payment request via Telegram Bot
2. **Order Creation**: Bot calls Balancer's `/orders` API with merchant info and amount
3. **Database Storage**: Balancer generates unique order ID and stores in database with PENDING status
4. **Payment Details**: Balancer returns order ID and payment details to Bot
5. **User Payment**: User pays to PaymentVault contract with order ID in transaction data
6. **Event Capture**: Indexer captures DepositReceived event from blockchain
7. **Status Update**: Indexer updates order status to COMPLETED in database
8. **Notifications**: Merchant can query status or receive notifications

## API Endpoints

### Core Endpoints

- `POST /orders` - Create new order
- `GET /orders/{order_id}` - Get order details
- `GET /accounts/{account_id}/balance` - Get account balance
- `GET /admin/orders` - List pending orders (admin only)
- `GET /health` - Health check

### Request/Response Examples

#### Create Order
```bash
curl -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{
    "account_id": "telegram_123456",
    "amount": "10.00",
    "token_address": "0xa0b86a33e6441d00000000000000000000000000"
  }'
```

#### Get Order Status
```bash
curl http://localhost:3000/orders/{order_id}
```

#### Get Account Balance
```bash
curl http://localhost:3000/accounts/{account_id}/balance
```

## Configuration

The service uses a YAML configuration file (`config.yaml`):

```yaml
database_url: "postgresql://localhost:5432/ledgerflow"
server:
  host: "127.0.0.1"
  port: 3000
business:
  max_pending_orders_per_account: 2
  broker_id: "ledgerflow-vault"
```

## Database Schema

### Orders Table
- `id`: UUID primary key
- `order_id`: Unique order identifier (keccak256 hash)
- `account_id`: Account identifier
- `broker_id`: Broker/merchant identifier
- `amount`: Payment amount (string for precision)
- `token_address`: ERC20 token contract address
- `status`: Order status (pending, completed, failed, cancelled)
- `created_at`, `updated_at`: Timestamps
- `transaction_hash`: Blockchain transaction hash (optional)

### Accounts Table
- `id`: UUID primary key
- `account_id`: Unique account identifier
- `email`: Email address (optional)
- `telegram_id`: Telegram user ID (optional)
- `evm_address`: Ethereum address (optional)
- `created_at`, `updated_at`: Timestamps

## Getting Started

### Prerequisites

- Rust 1.70+
- PostgreSQL 12+
- SQLx CLI: `cargo install sqlx-cli`

### Setup

1. Clone the repository
2. Copy and edit configuration:
   ```bash
   cp config.yaml.example config.yaml
   # Edit config.yaml with your database URL and settings
   ```

3. Set up database:
   ```bash
   # Create database
   createdb ledgerflow
   
   # Run migrations
   sqlx migrate run
   ```

4. Build and run:
   ```bash
   cargo build
   cargo run
   ```

### Development

#### Database Migrations

Create new migration:
```bash
sqlx migrate add <migration_name>
```

Run migrations:
```bash
sqlx migrate run
```

#### Testing

Run tests:
```bash
cargo test
```

#### Environment Variables

For development, you can set:
- `DATABASE_URL`: PostgreSQL connection string
- `RUST_LOG`: Logging level (debug, info, warn, error)

## Production Deployment

### Docker

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ledgerflow-balancer /usr/local/bin/
EXPOSE 3000
CMD ["ledgerflow-balancer"]
```

### Environment Variables

- `DATABASE_URL`: PostgreSQL connection string
- `RUST_LOG`: Logging configuration
- `CONFIG_FILE`: Path to configuration file

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

This project is licensed under the MIT License.
