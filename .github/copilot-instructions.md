# LedgerFlow AI Coding Assistant Instructions

## Project Overview

LedgerFlow is a blockchain-based payment gateway built on stablecoins (USDC) providing low-barrier, non-custodial payment solutions. The system uses a **decoupled architecture** where smart contracts serve as vaults, and multiple off-chain services handle business logic, event monitoring, and user interfaces.

## Architecture & Service Boundaries

### Core Components
- **ledgerflow-vault-evm/**: EVM smart contracts (Solidity/Foundry) - PaymentVault UUPS upgradeable contract for USDC deposits
- **ledgerflow-vault-aptos/**: Aptos smart contracts (Move) - alternative blockchain implementation
- **ledgerflow-balancer/**: Backend API service (Rust/Axum) - business logic core, order management, account system
- **ledgerflow-indexer-evm/**: EVM event monitoring (Rust/Alloy) - listens for DepositReceived events
- **ledgerflow-indexer-aptos/**: Aptos event monitoring (Rust) - monitors Move-based deposits
- **ledgerflow-bot/**: Telegram bot frontend (Rust/Teloxide) - user interface for payment requests
- **ledgerflow-eth-cli/**: Command-line tools (Rust/Clap) - EVM developer utilities
- **ledgerflow-aptos-cli/**: Command-line tools (Rust/Clap) - Aptos developer utilities
- **ledgerflow-aptos-cli-ts/**: TypeScript CLI (Bun/Commander.js) - modern Aptos interaction tool
- **ledgerflow-migrations/**: Database schema management (SQL) - unified PostgreSQL schema

### Critical Data Flow
1. **Order Creation**: Bot → Balancer API → Database (status: pending)
2. **Payment**: User → PaymentVault.deposit(orderId) → DepositReceived event
3. **Event Detection**: Indexer → processes event → Database (status: completed) 
4. **Notification**: Indexer → Bot → User notification

## Critical Implementation Details

### Order ID Generation Algorithm
**MUST** be consistent across all components:
```rust
// From ledgerflow-balancer/src/utils.rs:generate_order_id()
order_id = keccak256(abi.encodePacked(broker_id, account_id, order_id_num))
```
- Uses big-endian encoding (`to_be_bytes()`) to match Solidity's `abi.encodePacked`
- Pattern appears in balancer utils and must match smart contract expectations

### Database Schema Critical Patterns
- **Amounts**: Always `VARCHAR(255)` (never floats - arbitrary precision required)
- **Timestamps**: `TIMESTAMP WITH TIME ZONE` with auto-update triggers
- **Order Status**: PostgreSQL ENUM: `pending`, `deposited`, `completed`, `failed`, `cancelled`
- **Multi-chain**: `chain_id BIGINT` field in all relevant tables
- **Unique constraints**: Composite keys for chain_id + transaction_hash + log_index

### Smart Contract Architecture
- **UUPS Upgradeable**: Uses OpenZeppelin's UUPS pattern (`UUPSUpgradeable`)
- **Event Structure**: `DepositReceived(address indexed payer, bytes32 indexed orderId, uint256 amount)`
- **Both deposit modes**: Standard `approve/transferFrom` and `permit` for better UX
- **Deterministic deployment**: CREATE2 for consistent addresses across chains

## Development Workflows

### Build System (Just + Cargo Workspace)
```bash
# Root workspace commands
just format    # taplo fmt + cargo +nightly fmt --all
just lint      # strict clippy with -D clippy::unwrap_used + cargo machete
just test      # cargo test workspace-wide

# Component-specific builds
cd ledgerflow-{component} && cargo build

# TypeScript CLI (separate ecosystem)
cd ledgerflow-aptos-cli-ts && npm run build
cd ledgerflow-aptos-cli-ts && npm run dev    # Bun watch mode
```

### Database Management Pattern
```bash
# Run from ledgerflow-migrations/ only
cargo run -- migrate
# All services share same DATABASE_URL - modify schema carefully
```

### Multi-Chain Deployment
```bash
# EVM: Foundry with deterministic deployment
forge script script/DeployDeterministic.s.sol --rpc-url $RPC_URL --broadcast

# Aptos: Move commands via CLI tools
cd ledgerflow-vault-aptos && aptos move publish
# Or use TypeScript CLI for modern UX
cd ledgerflow-aptos-cli-ts && npm run build && node dist/index.js

# Configuration supports multiple chains simultaneously
# Each indexer instance monitors one chain/contract pair
```

## Project-Specific Conventions

### Configuration Pattern
- YAML files with `.example` templates in each component
- Config struct + `serde` + `config` crate pattern
- CLI `--config` argument overrides default `config.yaml`
- Environment variables override file values

### Workspace Dependencies
- All dependencies specified in root `Cargo.toml` `[workspace.dependencies]`
- Individual crates reference workspace versions to avoid conflicts
- Key patterns: `axum`, `tokio`, `sqlx`, `clap`, `config`, `tracing`, `eyre`
- Special handling for Aptos SDK via git dependencies with patches

### Error Handling Standard
- `eyre::Result` for all fallible operations
- Custom `AppError` per service with context wrapping
- Database operations always wrapped with operation context

### Logging Conventions
- `tracing` framework with structured fields
- Standard field names: `order_id`, `account_id`, `chain_id`, `transaction_hash`
- Log levels: database ops at info, business logic at info, errors at error

### Multi-Chain Support Pattern
- `chain_id` embedded in all data models and database tables
- Indexer config supports array of chain configurations
- Each chain has separate state tracking (`chain_states` table)
- Both EVM and Aptos chains use same database schema
