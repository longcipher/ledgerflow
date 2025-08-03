# LedgerFlow AI Coding Assistant Instructions

## Project Overview

LedgerFlow is a blockchain-based payment gateway built on stablecoins (USDC) providing low-barrier, non-custodial payment solutions. The system uses a **decoupled architecture** where smart contracts serve as vaults, and multiple off-chain services handle business logic, event monitoring, and user interfaces.

## Architecture & Service Boundaries

### Core Components
- **ledgerflow-vault-evm/**: EVM smart contracts (Solidity/Foundry) - PaymentVault UUPS upgradeable contract for USDC deposits
- **ledgerflow-vault-aptos/**: Aptos smart contracts (Move) - alternative blockchain implementation
- **ledgerflow-vault-sui/**: Sui smart contracts (Move) - object-based payment vault with capability control
- **ledgerflow-balancer/**: Backend API service (Rust/Axum) - business logic core, order management, account system
- **ledgerflow-indexer-evm/**: EVM event monitoring (Rust/Alloy) - listens for DepositReceived events
- **ledgerflow-indexer-aptos/**: Aptos event monitoring (Rust) - monitors Move-based deposits
- **ledgerflow-indexer-sui/**: Sui event monitoring (Rust/Sui SDK) - checkpoint-based event processing
- **ledgerflow-bot/**: Telegram bot frontend (Rust/Teloxide) - user interface for payment requests
- **ledgerflow-eth-cli/**: Command-line tools (Rust/Clap) - EVM developer utilities
- **ledgerflow-aptos-cli/**: Command-line tools (Rust/Clap) - Aptos developer utilities
- **ledgerflow-sui-cli/**: Command-line tools (Rust/Clap) - Sui interaction with dry-run support
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
- **UUPS Upgradeable (EVM)**: Uses OpenZeppelin's UUPS pattern (`UUPSUpgradeable`)
- **Resource-based (Aptos)**: Move resources with structured events
- **Object-based (Sui)**: `PaymentVault` objects with `OwnerCap` capability control
- **Event Structure**: `DepositReceived(address/account indexed payer, bytes32/vector<u8> indexed orderId, uint256/u64 amount)`
- **Multi-coin support**: EVM (approve/transferFrom + permit), Aptos (Fungible Assets), Sui (Coin<USDC> with Balance storage)
- **Deterministic deployment**: CREATE2 for EVM, consistent addresses across chains

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

# Sui: Move package deployment with automatic object sharing
cd ledgerflow-vault-sui && ./scripts/deploy.sh
# Or manually: sui client publish --gas-budget 100000000

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
- **Sui SDK**: Uses git dependencies from mystenlabs/sui with `sui-sdk` and `sui-json-rpc-types`

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
- **Sui Integration**: Checkpoint-based event processing with same database schema, using `sui_*` table prefixes

## Sui-Specific Implementation Details

### Object Model & Architecture
- **PaymentVault**: Shared object for public deposits, uses `Balance<USDC>` for efficient storage
- **OwnerCap**: Capability object bound to specific vault for admin operations
- **Event Access**: Uses `parsed_json` field instead of BCS for event data extraction
- **Clock Parameter**: Functions require `&Clock` object for timestamp management (`sui client call --args 0x6`)

### CLI Tools Pattern
- **Dry-run support**: `--dry-run` flag for transaction simulation without execution
- **Multiple output formats**: `--output json|pretty|compact` for different use cases
- **Configuration management**: YAML-based config with `.example` templates and environment variable overrides
- **Key handling**: Supports ed25519/secp256k1 with proper SuiAddress derivation from private keys

### Deployment Workflow
```bash
# Standard Sui deployment pattern used across vault and CLI tools
sui move build && sui client publish --gas-budget 100000000
# Object sharing for public access: sui client call --package sui --module transfer --function share_object
# Save deployment info to deployments/{network}.json for reference
```
