# Integration Guide: Using LedgerFlow Migrations Binary

This guide shows how to integrate the unified migration binary into each service.

## 1. Migration Strategy

Since `ledgerflow-migrations` is now a standalone binary, services should use it via:
- Direct execution of the migration binary
- Using the provided shell scripts
- Docker containers for production deployments

## 2. Remove Individual Migration Files

Delete the old migration directories from each service:
- `ledgerflow-balancer/migrations/`
- `ledgerflow-bot/migrations/`
- `ledgerflow-indexer/migrations/`

## 3. Update Service Code

### For all services (ledgerflow-balancer, ledgerflow-bot, ledgerflow-indexer)

Instead of using the migration manager in code, services should ensure migrations are run before starting:

#### Option 1: Using the migration script

```bash
# In your service startup script or Dockerfile
cd ../ledgerflow-migrations
./migrate.sh migrate
```

#### Option 2: Using the binary directly

```bash
# Set DATABASE_URL and run migrations
export DATABASE_URL="postgresql://postgres:password@localhost/ledgerflow"
cd ../ledgerflow-migrations
cargo run --bin ledgerflow-migrations
```

#### Option 3: Using the built binary

```bash
# Build once
cd ../ledgerflow-migrations
cargo build --release

# Run migrations
export DATABASE_URL="postgresql://postgres:password@localhost/ledgerflow"
./target/release/ledgerflow-migrations
```

## 4. Update Service Startup

### Using Docker Compose

Update docker-compose.yml to run migrations before starting services:

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
    volumes:
      - postgres_data:/var/lib/postgresql/data

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

volumes:
  postgres_data:
```

### Using Startup Scripts

Create a startup script for each service:

```bash
#!/bin/bash
# startup.sh for each service

set -e

# Run migrations first
cd ../ledgerflow-migrations
./migrate.sh migrate

# Then start the service
cd ../ledgerflow-balancer  # or appropriate service directory
cargo run --release
```

## 5. Environment Configuration

Ensure all services use the same database URL:

```bash
# .env file or environment variables
DATABASE_URL=postgresql://postgres:password@localhost/ledgerflow
```

## 6. Development Workflow

### Running Migrations

From the migrations directory:
```bash
cd ledgerflow-migrations
make migrate
```

### Adding New Migrations

```bash
cd ledgerflow-migrations
make add NAME="add_new_feature"
```

### Checking Migration Status

```bash
cd ledgerflow-migrations
make info
```

## 7. Docker Integration

Update docker-compose.yml to include migration initialization:

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
    volumes:
      - postgres_data:/var/lib/postgresql/data

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

volumes:
  postgres_data:
```

## 8. Testing

Create test utilities for integration tests:

```rust
// In tests/common/mod.rs
use ledgerflow_migrations::MigrationManager;
use sqlx::{Pool, Postgres};

pub async fn setup_test_db() -> Result<Pool<Postgres>, Box<dyn std::error::Error>> {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/ledgerflow_test".to_string());
    
    let migration_manager = MigrationManager::new(Some(&database_url)).await?;
    migration_manager.run_migrations().await?;
    
    Ok(migration_manager.get_pool().clone())
}
```

## 9. CI/CD Integration

Add migration steps to your CI pipeline:

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: password
          POSTGRES_DB: ledgerflow_test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432

    steps:
    - uses: actions/checkout@v2
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        
    - name: Install sqlx-cli
      run: cargo install sqlx-cli --no-default-features --features postgres
        
    - name: Run migrations
      run: |
        cd ledgerflow-migrations
        export DATABASE_URL=postgresql://postgres:password@localhost:5432/ledgerflow_test
        ./migrate.sh migrate
        
    - name: Run tests
      run: cargo test --all
```

## 10. Monitoring and Logging

Add migration monitoring to your services:

```rust
use tracing::{info, error};

pub async fn check_migration_status() -> Result<(), Box<dyn std::error::Error>> {
    let migration_manager = MigrationManager::new(None).await?;
    
    match migration_manager.check_migrations().await {
        Ok(versions) => {
            info!("Migration status check: {} migrations available", versions.len());
        }
        Err(e) => {
            error!("Migration status check failed: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}
```

This integration approach ensures all services share the same database schema while maintaining a clean separation of concerns.
