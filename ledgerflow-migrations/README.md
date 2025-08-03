# LedgerFlow Migrations

This crate provides unified database migration management for the entire LedgerFlow system.

## 📋 Project Status

**✅ COMPLETE** - Unified migration system ready for production use.

### Key Achievements
- ✅ **Unified Schema**: Consolidated migrations from all services (balancer, bot, indexer)
- ✅ **Production Ready**: Complete tooling with Docker, CI/CD, and shell scripts
- ✅ **Configuration Management**: YAML-based configuration with environment support
- ✅ **Testing**: Unit tests and integration validation
- ✅ **Documentation**: Comprehensive guides and integration instructions

### Architecture
```
ledgerflow-migrations/
├── src/
│   ├── lib.rs          # Core MigrationManager library
│   ├── main.rs         # CLI binary for running migrations
│   └── tests.rs        # Unit tests
├── migrations/
│   └── 20250709000001_initial_schema.sql  # Unified schema
├── config.yaml         # Configuration file
├── migrate.sh          # Shell script for operations
├── Makefile           # Build and operation commands
├── Dockerfile         # Container support
└── INTEGRATION.md     # Service integration guide
```

### Database Schema
The unified schema consolidates:
- **accounts** (from ledgerflow-balancer)
- **users** (from ledgerflow-bot)  
- **orders** (unified from all services)
- **chain_states** (from ledgerflow-indexer)
- **deposit_events** (from ledgerflow-indexer)
- **Optimized indexes** for common queries
- **Automatic triggers** for timestamp updates
- **ENUM types** for order status

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
- - Automatic `updated_at` timestamp updates for all tables

## Service Integration Guide

This migration system replaces individual service migrations. Services should use this unified system instead of managing their own migrations.

### Migration Strategy

Services should run migrations via:
1. **Direct binary execution**: Run the migration binary before service startup
2. **Shell scripts**: Use provided migration scripts in startup sequences  
3. **Docker containers**: Include migration step in container orchestration

### Updating Services

**Remove individual migration directories:**
- `ledgerflow-balancer/migrations/` ❌
- `ledgerflow-bot/migrations/` ❌ 
- `ledgerflow-indexer/migrations/` ❌

**Update service startup to run migrations first:**

#### Option 1: Using Migration Script
```bash
# In service startup script or Dockerfile
cd ../ledgerflow-migrations
./migrate.sh migrate
```

#### Option 2: Direct Binary Execution  
```bash
export DATABASE_URL="postgresql://postgres:password@localhost/ledgerflow"
cd ../ledgerflow-migrations
cargo run --bin ledgerflow-migrations
```

#### Option 3: Pre-built Binary
```bash
# Build once
cd ../ledgerflow-migrations
cargo build --release

# Run migrations
./target/release/ledgerflow-migrations
```

### Docker Compose Integration

```yaml
version: '3.8'
services:
  postgres:
    image: postgres:15
    environment:
      POSTGRES_DB: ledgerflow
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: password
    ports:
      - "5432:5432"

  migrations:
    build:
      context: .
      dockerfile: ledgerflow-migrations/Dockerfile
    depends_on:
      - postgres
    environment:
      - DATABASE_URL=postgresql://postgres:password@postgres:5432/ledgerflow
    command: ["./migrate.sh", "migrate"]

  balancer:
    build:
      context: .
      dockerfile: ledgerflow-balancer/Dockerfile
    depends_on:
      - migrations
    environment:
      - DATABASE_URL=postgresql://postgres:password@postgres:5432/ledgerflow
```

### Development Workflow

```bash
# Add new migrations
cd ledgerflow-migrations
make add NAME="add_new_feature"

# Run migrations
make migrate

# Check status
make info

# Reset (development only)
make reset
```

## Integration with Services

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
