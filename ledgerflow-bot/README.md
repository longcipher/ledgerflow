# LedgerFlow Bot

LedgerFlow Bot is a Telegram bot that serves as the primary user interface for the LedgerFlow payment system. It allows users to create payment requests, manage their wallets, and track their payment history through a conversational interface.

## 📋 Project Status

**✅ COMPLETED - READY FOR DEVELOPMENT** - Foundation established with core infrastructure ready.

### Current Development Phase
- **Phase 1: Foundation** ✅ COMPLETED - Project structure, CLI interface, and documentation
- **Phase 2: Core Features** ⏳ IN PROGRESS - Database integration and bot functionality
- **Phase 3: Advanced Features** ⏳ PLANNED - Payment requests and order tracking
- **Phase 4: Production Ready** ⏳ PLANNED - Security hardening and deployment

### Technology Stack
- **Rust 2021**: Core language with async/await support
- **Teloxide**: Telegram bot framework
- **SQLx**: Database ORM with PostgreSQL
- **Tokio**: Async runtime
- **Clap**: CLI argument parsing
- **Alloy**: EVM blockchain interactions
- **Reqwest**: HTTP client for API calls

### Architecture Overview
```
ledgerflow-bot/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── config.rs         # Configuration management
│   ├── database.rs       # PostgreSQL database layer
│   ├── error.rs          # Error types and handling
│   ├── models.rs         # Data models and types
│   ├── handlers.rs       # Telegram bot handlers
│   ├── wallet.rs         # EVM wallet utilities
│   ├── services.rs       # External service integration
│   └── bot.rs            # Bot UI utilities
├── migrations/
│   └── 001_initial.sql   # Database schema
├── config.yaml           # Configuration file
└── Makefile              # Development tools
```

### Completed Features
- ✅ **Project Structure**: All dependencies enabled and configured
- ✅ **CLI Interface**: Fully functional command-line interface
- ✅ **Database Layer**: PostgreSQL integration with user management
- ✅ **Configuration System**: YAML-based configuration
- ✅ **Error Handling**: Comprehensive error types
- ✅ **Bot Infrastructure**: Message handlers and callback support
- ✅ **Wallet Utilities**: EVM wallet generation and validation
- ✅ **Documentation**: Complete setup and usage guides

## Features

- **Session-based registration**: Users register via a conversational flow (email → username → wallet auto-generated)
- **Menu-driven UX**: All actions (deposit, withdraw, account info) are accessible via inline keyboard menus
- **Fully-custodial wallet**: Each user gets a unique, encrypted wallet managed by the system
- **Admin-only withdraw**: Only admin users can trigger withdrawals
- **Deposit flow**: Users can deposit by entering an amount, receiving an order, and sending funds
- **Order notifications**: Users are notified when their deposit is confirmed
- **Stateful user sessions**: The bot remembers where each user is in the flow
- **English-only interface**: All prompts, errors, and menus are in English
- **Command-line configuration**: Specify a custom config file path via CLI arguments

## Usage

Run the bot with the default configuration:

```bash
cargo run
```

Specify a custom configuration file:

```bash
cargo run -- --config /path/to/custom-config.yaml
```

View all available options:

```bash
cargo run -- --help
```

## Technology Stack

- **Rust**: Core language for performance and safety
- **Teloxide**: Telegram Bot API framework
- **SQLx**: Database ORM with compile-time SQL checking
- **Alloy**: Ethereum/EVM blockchain interaction
- **PostgreSQL**: Database for user data and order tracking
- **Reqwest**: HTTP client for API communication
- **Tracing**: Structured logging and observability

## Development Status

### Current Capabilities
- ✅ **CLI Interface**: Fully functional with start, generate-wallet, and version commands
- ✅ **Project Compilation**: Clean build with all dependencies properly configured
- ✅ **Database Schema**: PostgreSQL migrations for users and orders
- ✅ **Configuration**: YAML-based configuration management
- ✅ **Error Handling**: Comprehensive error types and propagation

### Next Steps
1. **Database Integration Testing**: Set up PostgreSQL and test connections
2. **Telegram Bot Testing**: Configure bot token and test API interactions
3. **Service Integration**: Test Balancer service integration and API calls
4. **Command Implementation**: Implement and test all bot commands
5. **Security Review**: Validate private key handling and input sanitization

### Development Commands
```bash
# Build and run
cargo build
cargo run -- start

# Generate wallet (placeholder)
cargo run -- generate-wallet

# Show help and version
cargo run -- --help
cargo run -- --version

# Development tools
make setup    # Setup development environment
make test     # Run tests (when implemented)
make docs     # Generate documentation
```

## Architecture

### User Flow

1. **Registration**: User starts with `/start` or the bot's start button
   - Bot asks for email
   - Bot asks for username
   - Bot creates a custodial wallet and account
   - Bot shows account info and main menu
2. **Main Menu**: User can choose:
   - Deposit (enter amount, get order, send funds)
   - Withdraw (admin only)
   - View account info
   - Return to main menu
3. **Deposit**:
   - User enters amount
   - Bot creates order and shows deposit address/order ID
   - User sends funds
   - Bot notifies user when deposit is confirmed
4. **Withdraw**:
   - Only available to admin users
   - Admin can trigger withdrawal for a user
5. **Notifications**:
   - Bot periodically checks for completed orders and notifies users

All flows are stateful and menu-driven, with clear English prompts and error messages. Legacy commands and Chinese prompts have been removed.
