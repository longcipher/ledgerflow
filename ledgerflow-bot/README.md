# LedgerFlow Bot

LedgerFlow Bot is a Telegram bot that serves as the primary user interface for the LedgerFlow payment system. It allows users to create payment requests, manage their wallets, and track their payment history through a conversational interface.

## Features

### Core Functionality
- **🚀 User Onboarding**: Guide users through account setup and wallet binding
- **💳 Payment Requests**: Create payment orders with unique order IDs
- **📊 Balance Queries**: Check total received payments and balance
- **👛 Wallet Management**: Generate new wallets or bind existing addresses
- **📋 Order Tracking**: View payment history and order status
- **🔔 Notifications**: Receive updates when payments are completed

### Security Features
- **🔐 Private Key Management**: Secure wallet generation and storage
- **✅ Address Validation**: Validate EVM addresses before binding
- **🛡️ Input Sanitization**: Prevent injection attacks and validate user inputs
- **🔒 Database Security**: Encrypted database connections

## Technology Stack

- **Rust**: Core language for performance and safety
- **Teloxide**: Telegram Bot API framework
- **SQLx**: Database ORM with compile-time SQL checking
- **Alloy**: Ethereum/EVM blockchain interaction
- **PostgreSQL**: Database for user data and order tracking
- **Reqwest**: HTTP client for API communication
- **Tracing**: Structured logging and observability

## Architecture

### Bot Commands

#### Basic Commands
- `/start` - Initialize the bot and show welcome message
- `/help` - Display all available commands
- `/balance` - Check your current balance
- `/wallet` - View your wallet information

#### Payment Commands
- `/pay <amount>` - Create a payment request
  - Example: `/pay 10.5`
- `/bind <address>` - Bind your EVM address
  - Example: `/bind 0x742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6`
- `/generate_wallet` - Generate a new wallet address

### User Flow

1. **Registration**: User starts bot with `/start`
2. **Wallet Setup**: User binds existing address or generates new wallet
3. **Payment Creation**: User creates payment request with `/pay <amount>`
4. **Payment Details**: Bot returns payment address, amount, and order ID
5. **Payment Execution**: User sends payment to provided address
6. **Notification**: Bot notifies user when payment is confirmed
7. **Balance Update**: User can check updated balance with `/balance`

### Integration Points

- **Balancer API**: Creates orders and queries balances
- **Database**: Stores user data and caches order information
- **Blockchain**: Monitors payment events and transaction status

## Configuration

The bot uses a YAML configuration file:

```yaml
database_url: "postgresql://localhost:5432/ledgerflow"

telegram:
  bot_token: "YOUR_BOT_TOKEN_HERE"
  webhook_url: null  # Optional: for webhook mode

balancer:
  base_url: "http://localhost:3000"
  timeout_seconds: 30

blockchain:
  rpc_url: "https://sepolia.unichain.org"
  payment_vault_address: "0x0000000000000000000000000000000000000000"
  chain_id: 1301
```

## Database Schema

### Users Table
- `id`: UUID primary key
- `telegram_id`: Unique Telegram user ID
- `username`: Telegram username (optional)
- `first_name`: User's first name
- `last_name`: User's last name
- `evm_address`: Bound EVM address (optional)
- `created_at`, `updated_at`: Timestamps

### Orders Table (Shared with Balancer)
- `id`: UUID primary key
- `order_id`: Unique order identifier
- `account_id`: Account identifier (Telegram ID)
- `broker_id`: Broker/merchant identifier
- `amount`: Payment amount
- `token_address`: Token contract address
- `status`: Order status (pending, completed, failed, cancelled)
- `created_at`, `updated_at`: Timestamps
- `transaction_hash`: Blockchain transaction hash

## Getting Started

### Prerequisites

- Rust 1.70+
- PostgreSQL 12+
- Telegram Bot Token (from [@BotFather](https://t.me/BotFather))
- Running LedgerFlow Balancer service

### Installation

1. Clone the repository:
   ```bash
   git clone <repository-url>
   cd ledgerflow-bot
   ```

2. Install dependencies:
   ```bash
   cargo build
   ```

3. Set up configuration:
   ```bash
   cp config.yaml.example config.yaml
   # Edit config.yaml with your settings
   ```

4. Set up database:
   ```bash
   # Create database
   createdb ledgerflow
   
   # Run migrations
   sqlx migrate run
   ```

5. Get Telegram Bot Token:
   - Message [@BotFather](https://t.me/BotFather) on Telegram
   - Use `/newbot` command to create a new bot
   - Copy the bot token to your config.yaml

### Running the Bot

```bash
# Start the bot
cargo run -- start

# Generate a wallet (utility command)
cargo run -- generate-wallet

# Show version
cargo run -- version
```

### Environment Variables

- `DATABASE_URL`: PostgreSQL connection string
- `RUST_LOG`: Logging level (debug, info, warn, error)
- `CONFIG_FILE`: Path to configuration file

## Usage Examples

### Creating a Payment Request

```
User: /pay 25.50
Bot: 💳 Payment Request Created:

Order ID: 0x1234567890abcdef...
Amount: 25.50 USDC
Payment Address: 0x742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6
Chain: Unichain Sepolia

Send the exact amount to the payment address with the Order ID in the transaction data.
```

### Checking Balance

```
User: /balance
Bot: 💰 Your Balance:

Total: 150.75 USDC
Account: telegram_123456

Use /pay <amount> to create a payment request
```

### Generating a Wallet

```
User: /generate_wallet
Bot: 🆕 Generated New Wallet:

Address: 0x742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6
Private Key: 0x1234567890abcdef...

⚠️ IMPORTANT: Keep your private key secure!
Use /bind 0x742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6 to bind this address
```

## Development

### Running Tests

```bash
cargo test
```

### Adding New Commands

1. Add handler function in `src/handlers.rs`
2. Add command pattern matching in `handle_message`
3. Add command to help text in `handle_help`
4. Add any new models to `src/models.rs`

### Database Migrations

Create new migration:
```bash
sqlx migrate add <migration_name>
```

### Logging

The bot uses structured logging with tracing:

```bash
# Enable debug logging
RUST_LOG=debug cargo run -- start

# Enable info logging with specific modules
RUST_LOG=ledgerflow_bot=info,sqlx=warn cargo run -- start
```

## Production Deployment

### Docker Deployment

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/ledgerflow-bot /usr/local/bin/
COPY --from=builder /app/config.yaml.example /etc/ledgerflow/config.yaml

EXPOSE 8080
CMD ["ledgerflow-bot", "start", "--config", "/etc/ledgerflow/config.yaml"]
```

### Environment Variables

```bash
DATABASE_URL=postgresql://user:password@localhost:5432/ledgerflow
RUST_LOG=info
CONFIG_FILE=/etc/ledgerflow/config.yaml
```

### Webhook Mode

For production, you may want to use webhooks instead of polling:

```yaml
telegram:
  bot_token: "YOUR_BOT_TOKEN"
  webhook_url: "https://your-domain.com/webhook"
```

## Security Considerations

1. **Private Key Storage**: Never log or expose private keys
2. **Input Validation**: Always validate user inputs
3. **Rate Limiting**: Implement rate limiting for API calls
4. **Database Security**: Use encrypted connections and parameterized queries
5. **Error Handling**: Don't expose internal errors to users
6. **Monitoring**: Monitor for suspicious activity and errors

## Troubleshooting

### Common Issues

1. **Database Connection Failed**
   - Check DATABASE_URL format
   - Ensure PostgreSQL is running
   - Verify database exists

2. **Telegram API Errors**
   - Verify bot token is correct
   - Check bot permissions
   - Ensure bot is not blocked

3. **Balancer API Errors**
   - Check balancer service is running
   - Verify API endpoint URLs
   - Check network connectivity

### Debugging

Enable debug logging:
```bash
RUST_LOG=debug cargo run -- start
```

Check database connection:
```bash
psql $DATABASE_URL -c "SELECT 1"
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Support

For support and questions:
- Create an issue in the repository
- Join our community Discord
- Check the documentation

---

**Note**: This bot handles financial transactions. Always test thoroughly in a development environment before deploying to production.
