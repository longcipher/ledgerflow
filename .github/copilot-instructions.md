# LedgerFlow AI Coding Assistant Instructions

## Project Overview

LedgerFlow is a blockchain-based payment gateway built on stablecoins (USDC) to provide low-barrier, non-custodial payment solutions. The system uses a **decoupled architecture** where a single smart contract serves as the vault, and multiple off-chain services handle business logic, event monitoring, and user interfaces.

## Architecture & Service Boundaries

### Core Components
- **ledgerflow-vault-evm/**: EVM smart contracts (Solidity/Foundry) - single PaymentVault contract that receives all USDC deposits
- **ledgerflow-vault-aptos/**: Aptos smart contracts (Move) - alternative blockchain implementation
- **ledgerflow-balancer/**: Backend API service (Rust/Axum) - business logic core, order management, account system
- **ledgerflow-indexer/**: Event monitoring service (Rust/Alloy) - listens for DepositReceived events, updates order status
- **ledgerflow-bot/**: Telegram bot frontend (Rust/Teloxide) - user interface for payment requests and notifications
- **ledgerflow-cli/**: Command-line tools (Rust/Clap) - developer utilities
- **ledgerflow-migrations/**: Database schema management (SQL) - unified PostgreSQL schema for all services

### Data Flow Pattern
1. **Order Creation**: Bot → Balancer API → Database (status: pending)
2. **Payment**: User → PaymentVault contract (with orderId)
3. **Event Detection**: Indexer → DepositReceived event → Database (status: completed)
4. **Notification**: Indexer → Bot → User notification

## Critical Implementation Details

### Order ID Generation Algorithm
All components use the same keccak256-based order ID generation:
```rust
// From ledgerflow-balancer/src/utils.rs
order_id = keccak256(abi.encodePacked(broker_id, account_id, order_id_num))
```
- This pattern appears in `ledgerflow-balancer/src/utils.rs:generate_order_id()` and must be consistent across all components
- Uses big-endian encoding for numeric values to match Solidity's abi.encodePacked

### Database Schema Patterns
- Use `VARCHAR(255)` for amounts (arbitrary precision handling, no floating point)
- All timestamps are `TIMESTAMP WITH TIME ZONE`
- Order status uses PostgreSQL ENUM: `pending`, `deposited`, `completed`, `failed`, `cancelled`
- Chain ID support built into all tables for multi-chain deployments
- See `ledgerflow-migrations/migrations/001_initial_schema.sql` for complete schema

### Error Handling Convention
- Use `eyre::Result` for error propagation in all Rust components
- Custom `AppError` types per service (see `*/src/error.rs`)
- Database errors wrapped with context about the failing operation

## Development Workflows

### Build & Test Commands
```bash
# Use Just for common tasks (workspace root)
just format    # Format all code (cargo fmt + taplo fmt)
just lint      # Full linting pipeline with strict rules including clippy::unwrap_used
just test      # Run all tests

# Per-component builds
cd ledgerflow-{component} && cargo build --release
```

### Database Management
```bash
# Migrations run from ledgerflow-migrations/
cargo run -- migrate

# Each service needs DATABASE_URL environment variable
# Schema is shared across all services - modify migrations/ carefully
```

### Smart Contract Deployment
```bash
# EVM contracts from ledgerflow-vault-evm/
forge script script/DeployDeterministic.s.sol --rpc-url $RPC_URL --broadcast
# Uses CREATE2 for deterministic addresses across chains

# Move contracts from ledgerflow-vault-aptos/
# See individual README files for Move-specific deployment
```

## Project-Specific Conventions

### Configuration Management
- All services use YAML config files with `.example` templates
- Config structs use `serde` with `config` crate pattern
- Environment variables override config file values
- Each service has CLI argument for custom config path: `--config`

### Logging & Observability
- Standardized on `tracing` framework across all Rust components
- Structured logging with consistent field names: `order_id`, `account_id`, `chain_id`
- Database operations always logged with context
- Use info!/warn!/error! macros consistently

### Multi-Chain Support Pattern
- Chain ID embedded in all data models and database tables (`chain_id` field)
- Indexer runs separate monitoring loops per chain/contract pair
- Configuration supports multiple chain endpoints and contract addresses
- Both EVM and non-EVM (Aptos) chains supported

### Security Patterns
- Smart contract uses UUPS upgradeable pattern with OpenZeppelin
- Bot manages encrypted private keys for user wallets (custodial model using XOR encryption)
- Balancer validates business rules (max 2 pending orders per account)
- All services validate order ownership before state changes

## Integration Points

### API Contracts
- Balancer exposes REST API consumed by Bot: `/orders`, `/accounts`, `/balances`
- Request/response models in `*/src/models.rs` with consistent field naming
- All APIs return JSON with standardized error format

### Smart Contract Events
```solidity
event DepositReceived(address indexed payer, bytes32 indexed orderId, uint256 amount);
```
- Indexer processes this event to complete payment flow using keccak256 signature matching
- Event data maps directly to database order records
- Must handle duplicate events (idempotent processing)

### Database Constraints
- Foreign key relationships: `orders.account_id` → `accounts.id`
- Unique constraints on `order_id`, `telegram_id`, `username`
- Use transactions for multi-table operations (order creation + balance updates)

## Key Files for Understanding Patterns

- `ledgerflow-balancer/src/models.rs` - Core data models shared across services
- `ledgerflow-balancer/src/utils.rs` - Order ID generation and encryption utilities
- `ledgerflow-migrations/migrations/001_initial_schema.sql` - Complete database schema
- `ledgerflow-vault-evm/src/PaymentVault.sol` - EVM smart contract interface
- `ledgerflow-indexer/src/indexer.rs` - Event monitoring patterns and multi-chain handling
- `Cargo.toml` (workspace root) - Shared dependency versions and features

## Testing Approach

- Unit tests for business logic (order generation, validation)
- Integration tests require PostgreSQL database
- Smart contract tests use Foundry framework in `ledgerflow-vault-evm/test/`
- Bot testing uses mock HTTP clients for Balancer API calls
- Clippy configured to forbid `.unwrap()` usage - use proper error handling
