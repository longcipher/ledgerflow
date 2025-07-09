#!/bin/bash

# LedgerFlow Migration System Demo
# This script demonstrates the unified migration system usage

set -e

echo "🚀 LedgerFlow Migration System Demo"
echo "=================================="

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_step() {
    echo -e "${BLUE}📋 $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Change to migrations directory
cd ledgerflow-migrations

print_step "Step 1: Check if sqlx-cli is installed"
if command -v sqlx &> /dev/null; then
    print_success "sqlx-cli is installed"
else
    print_warning "sqlx-cli not found. Installing..."
    make install
fi

print_step "Step 2: Show available migration commands"
echo "Available commands:"
echo "  make migrate  - Run migrations"
echo "  make info     - Show migration status"
echo "  make add      - Add new migration"
echo "  make reset    - Reset database"
echo ""

print_step "Step 3: Build the migration manager"
cargo build --release
print_success "Migration manager built successfully"

print_step "Step 4: Run tests"
cargo test
print_success "All tests passed"

print_step "Step 5: Show migration files"
echo "Migration files:"
ls -la migrations/
echo ""

print_step "Step 6: Show configuration"
echo "Configuration file:"
cat config.yaml
echo ""

print_step "Step 7: Database connection test"
if [ -z "$DATABASE_URL" ]; then
    print_warning "DATABASE_URL not set. Using default test connection"
    export DATABASE_URL="postgresql://postgres:password@localhost/ledgerflow_test"
fi

echo "Database URL: $DATABASE_URL"

print_step "Step 8: Test migration manager"
echo "Testing migration manager (requires database connection)..."
./target/release/ledgerflow-migrations 2>/dev/null || {
    print_warning "Database connection failed (this is expected if no database is running)"
    print_warning "To run migrations, ensure PostgreSQL is running and DATABASE_URL is set"
}

print_step "Step 9: Show integration examples"
echo ""
echo "Integration examples:"
echo ""
echo "1. In your service's Cargo.toml:"
echo "   [dependencies]"
echo "   ledgerflow-migrations = { path = \"../ledgerflow-migrations\" }"
echo ""
echo "2. In your service's main.rs:"
echo "   use ledgerflow_migrations::MigrationManager;"
echo ""
echo "   let manager = MigrationManager::new(Some(&database_url)).await?;"
echo "   manager.run_migrations().await?;"
echo ""

print_step "Step 10: Show Docker usage"
echo "Docker usage:"
echo "  docker build -t ledgerflow-migrations ."
echo "  docker run -e DATABASE_URL=... ledgerflow-migrations"
echo ""

print_success "Demo completed successfully!"
echo ""
echo "🎯 Next Steps:"
echo "1. Set up your PostgreSQL database"
echo "2. Set DATABASE_URL environment variable"
echo "3. Run 'make setup' to create database and run migrations"
echo "4. Integrate with your services using the examples above"
echo ""
echo "📚 Documentation:"
echo "- README.md: Complete usage guide"
echo "- INTEGRATION.md: Service integration guide"
echo "- PROJECT_STATUS.md: Current status and roadmap"
echo ""
echo "🔧 Common Commands:"
echo "- make migrate: Run pending migrations"
echo "- make add NAME='add_new_table': Add new migration"
echo "- make info: Show migration status"
echo "- make reset: Reset database (development only)"
