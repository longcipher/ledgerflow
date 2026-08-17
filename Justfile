# Default recipe to display help
default:
  @just --list

# Format all code
format:
  rumdl fmt .
  cargo sort -w -g
  cargo +nightly fmt --all

# Auto-fix linting issues
fix:
  rumdl check --fix .

# Run all lints
lint:
  typos
  rumdl check .
  cargo sort -w -g -c
  cargo +nightly fmt --all -- --check
  cargo +nightly clippy --all -- -D warnings
  cargo shear

# Run tests
test:
  cargo test --all-features

# Run all test suites
test-all:
  cargo test --all-features

# Run tests with coverage
test-coverage:
  cargo tarpaulin --all-features --workspace --timeout 300

# Run mutation testing to validate test quality
mutate:
  cargo mutants --all-features

# Benchmark the merchant verification hot path
bench:
  cargo bench -p ledgerflow-core

# Type-check the fuzz targets without running an open-ended fuzz session
fuzz-check:
  cd crates/ledgerflow-core && cargo fuzz check decode_warrant
  cd crates/ledgerflow-protocol && cargo fuzz check parse_challenge
  cd crates/ledgerflow-protocol && cargo fuzz check parse_authorization

# Run short fuzzing smoke tests for the protocol boundaries
fuzz-smoke:
  cd crates/ledgerflow-core && cargo fuzz run decode_warrant -- -max_total_time=1
  cd crates/ledgerflow-protocol && cargo fuzz run parse_challenge -- -max_total_time=1

# Build entire workspace
build:
  cargo build --workspace

# Check all targets compile
check:
  cargo check --all-targets --all-features

# Publish all crates to crates.io (dry run)
publish-check:
  cargo publish --workspace --dry-run --allow-dirty

# Publish all crates to crates.io
publish:
  cargo publish --workspace

# Check for Chinese characters
check-cn:
  rg --line-number --column "\p{Han}"

# Full CI check
ci: lint test-all build

# ============================================================
# Maintenance & Tools
# ============================================================

# Clean build artifacts
clean:
  cargo clean

# Install all required development tools
setup:
  cargo install cargo-shear
  cargo install cargo-sort
  cargo install cargo-mutants
  cargo install typos-cli

# Generate documentation for the workspace
docs:
  cargo doc --no-deps --open
