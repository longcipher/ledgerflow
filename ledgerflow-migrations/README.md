# LedgerFlow Migrations

This crate provides unified database migration management for the entire LedgerFlow system.

## Overview

This crate consolidates all database migrations from the individual services:
- `ledgerflow-balancer`
- `ledgerflow-bot` 
- `ledgerflow-indexer`

All migrations are now managed centrally to avoid conflicts and ensure consistency across the entire system.

## Usage

### Prerequisites

Install sqlx-cli:
```bash
cargo install sqlx-cli --no-default-features --features postgres
```

### Environment Variables

Set the database URL:
```bash
export DATABASE_URL="postgresql://username:password@localhost/ledgerflow"
```

### Using the Migration Script

The `migrate.sh` script provides convenient commands for managing migrations:

```bash
# Setup database (create + run migrations)
./migrate.sh setup

# Run pending migrations
./migrate.sh migrate

# Add new migration
./migrate.sh add "add_new_table"

# Show migration status
./migrate.sh info

# Revert migrations (default: 1 step)
./migrate.sh revert
./migrate.sh revert 3  # revert 3 steps

# Reset database (drop + recreate + run all migrations)
./migrate.sh reset
```

### Using sqlx-cli Directly

You can also use sqlx-cli commands directly:

```bash
# Run migrations
sqlx migrate run

# Add new migration
sqlx migrate add "migration_name"

# Show migration info
sqlx migrate info

# Revert migrations
sqlx migrate revert
```

### Using as a Binary

The migrations crate is now a standalone binary application. You can run it directly:

```bash
# Run migrations
cargo run --bin ledgerflow-migrations

# Or build and run the binary
cargo build --release
./target/release/ledgerflow-migrations
```

### Integration with Services

Since this is now a binary-only crate, services should use the migration binary or script for database initialization:

```bash
# In your service startup script
cd ../ledgerflow-migrations
./migrate.sh migrate
```

Or you can copy the migration structures to your service if you need programmatic access (not recommended for most use cases).

## Migration Files

All migration files are located in the `migrations/` directory and follow the naming convention:
```
YYYYMMDDHHMMSS_description.sql
```

## Schema Overview

The unified schema includes:

### Tables
- `accounts` - User accounts (from balancer)
- `users` - Telegram users (from bot)
- `orders` - Order management (unified from all services)
- `chain_states` - Blockchain scanning state (from indexer)
- `deposit_events` - Deposit event logs (from indexer)

### Types
- `order_status` - ENUM for order statuses

### Triggers
- Automatic `updated_at` timestamp updates for all tables

## Development

When adding new migrations:

1. Use descriptive names for migration files
2. Include both `up` and `down` migration logic when possible
3. Test migrations on a development database first
4. Use `IF NOT EXISTS` clauses when appropriate to avoid conflicts

## Integration with Services

Each service should use this crate for database initialization:

```toml
[dependencies]
ledgerflow-migrations = { path = "../ledgerflow-migrations" }
```

Then in your service startup code:
```rust
use ledgerflow_migrations::MigrationManager;

async fn setup_database() -> Result<sqlx::PgPool, sqlx::Error> {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let migration_manager = MigrationManager::new(&database_url).await?;
    migration_manager.run_migrations().await?;
    Ok(migration_manager.get_pool().clone())
}
```
