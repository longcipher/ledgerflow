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

## 📋 Project Status

**✅ COMPLETE** - Initial implementation with all core features functional.

### Completed Features
- ✅ **Core Architecture**: Rust project with Axum web framework and SQLx database integration
- ✅ **Account Management**: Full account registration and lookup by username/email/telegram ID
- ✅ **Order System**: Order creation, status tracking, and balance aggregation
- ✅ **Business Logic**: Order ID generation using keccak256 algorithm
- ✅ **Database Layer**: PostgreSQL schema with orders and accounts tables
- ✅ **API Endpoints**: Complete REST API with comprehensive error handling
- ✅ **Configuration**: YAML-based configuration management
- ✅ **Logging**: Enhanced logging with emojis and detailed tracking
- ✅ **Documentation**: Comprehensive API documentation and usage examples

### Technology Achievements
- **Language**: Rust 2021 Edition
- **Web Framework**: Axum 0.7 with routing and CORS
- **Database**: PostgreSQL with SQLx 0.8 and connection pooling
- **CLI**: Clap 4.0 for argument parsing
- **Config**: YAML-based configuration with environment overrides
- **Logging**: Tracing with structured output and Unicode emojis
- **Error Handling**: Eyre + thiserror for comprehensive error management
- **Crypto**: SHA3 for secure order ID generation
- **Async**: Tokio runtime for concurrent request handling

### Project Structure
```
ledgerflow-balancer/
├── src/
│   ├── main.rs           # Application entry point with server setup
│   ├── config.rs         # YAML configuration management
│   ├── database.rs       # PostgreSQL database layer
│   ├── error.rs          # Comprehensive error handling
│   ├── handlers.rs       # HTTP request handlers
│   ├── models.rs         # Data models and request/response types
│   ├── services.rs       # Business logic services
│   └── utils.rs          # Utility functions (order ID generation)
├── migrations/
│   └── 001_initial.sql   # Database schema with proper indexes
├── config.yaml           # Runtime configuration
├── Cargo.toml            # Dependencies and project configuration
├── Justfile              # Development workflow commands
└── example.sh            # API usage examples script
```

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

- `POST /register` - Register new account
- `GET /accounts/username/{username}` - Get account by username
- `GET /accounts/email/{email}` - Get account by email
- `GET /accounts/telegram/{telegram_id}` - Get account by telegram ID
- `POST /orders` - Create new order
- `GET /orders/{order_id}` - Get order details
- `GET /accounts/{account_id}/balance` - Get account balance
- `GET /admin/orders` - List pending orders (admin only)
- `GET /health` - Health check

### x402 Facilitator Endpoints

- `GET /x402/supported` - List supported kinds (scheme/network)
- `POST /x402/verify` - Verify an x402 payment header against requirements
- `POST /x402/settle` - Settle an x402 payment (EVM exact via EIP-3009 wrapper)

Enable these by adding the `x402` section to `config.yaml` (see `config.yaml.example`).

### Request/Response Examples

#### Register Account
```bash
curl -X POST http://localhost:3000/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "john_doe",
    "email": "john@example.com",
    "telegram_id": 123456789,
    "evm_address": "0x1234567890abcdef1234567890abcdef12345678"
  }'
```

#### Get Account by Username
```bash
curl http://localhost:3000/accounts/username/john_doe
```

#### Get Account by Email
```bash
curl http://localhost:3000/accounts/email/john@example.com
```

#### Get Account by Telegram ID
```bash
curl http://localhost:3000/accounts/telegram/123456789
```

#### Create Order
```bash
curl -X POST http://localhost:3000/orders \
  -H "Content-Type: application/json" \
  -d '{
    "account_id": 1,
    "amount": "10.00",
    "token_address": "0xa0b86a33e6441d00000000000000000000000000",
    "chain_id": 1
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

## Enhanced Logging System

The service includes comprehensive logging with emojis for better visibility:

### Startup Phase Logs
- 🚀 **Program Startup**: Service initialization
- 📋 **Configuration Loading**: YAML config file loading status
- 🔗 **Database Connection**: PostgreSQL connection status
- 🔄 **Background Tasks**: Order processing task startup
- 🏗️ **Route Building**: Application routing setup
- 🌐 **Service Binding**: Server bind address
- 🎯 **Service Ready**: Service ready status
- 💡 **Endpoint List**: Available API endpoints

### Request Processing Logs
- 📝 **API Requests**: Various API request processing
- 🏥 **Health Checks**: Health check requests
- 👤 **Account Registration**: Account registration process
- 📦 **Order Creation**: Order creation process
- 💰 **Balance Queries**: Balance query requests

### Background Task Logs
- 🔄 **Task Loops**: Background task loop status
- ⏸️ **Idle State**: No orders to process state
- ✅ **Success Processing**: Successful order processing
- ❌ **Processing Failures**: Failed order processing
- 📊 **Batch Statistics**: Batch processing completion stats

### Log Level Configuration
```bash
# Environment variable
export RUST_LOG=info

# Available levels: error, warn, info, debug, trace
RUST_LOG=info cargo run --bin ledgerflow-balancer
```

### Example Log Output
```
🚀 LedgerFlow Balancer starting up...
📋 Loading configuration from config.yaml
✅ Configuration loaded successfully
🔗 Connecting to database...
✅ Database connected successfully
🔄 Starting background task for processing deposited orders...
🎯 LedgerFlow Balancer is ready and listening on 0.0.0.0:8080
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
