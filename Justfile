# Default recipe to display help
default:
  @just --list

# Format all code: markdown, Cargo.toml ordering, rustfmt
format:
  rumdl fmt .
  cargo sort -w -g
  cargo +nightly fmt --all

# Auto-fix what `format` cannot: markdown lint, clippy, unused deps
fix:
  rumdl check --fix .
  RUSTC_WRAPPER= cargo +nightly clippy --fix --all --allow-dirty
  cargo shear --fix

# Run all lints (the CI gate: typos, rumdl, cargo sort, rustfmt, clippy, shear)
lint:
  typos
  rumdl check .
  cargo sort -w -g -c
  cargo +nightly fmt --all -- --check
  RUSTC_WRAPPER= cargo +nightly clippy --all -- -D warnings
  cargo shear

# Run tests
test:
  cargo test --all-features

# Run every test suite in the workspace
test-all:
  cargo test --all-features

# Run mutation testing with cargo-mutants
mutation:
  cargo mutants --all-features

# Run mutation testing with cargo-mutants (legacy alias for `mutation`)
mutate: mutation

# Run tests with coverage
test-coverage:
  cargo tarpaulin --all-features --workspace --timeout 300

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

# Publish all crates to crates.io (irreversible; prompts only on an interactive terminal)
publish:
  #!/usr/bin/env bash
  set -euo pipefail
  if [ -t 0 ]; then
    read -r -p "Type 'publish' to continue: " confirm
    if [ "$confirm" != "publish" ]; then
      echo "aborted"
      exit 1
    fi
  fi
  cargo publish --workspace

# Check for Chinese characters (the Chinese design doc and README are excluded)
check-cn:
  ! rg --line-number --column "\p{Han}" --glob '!README.md' --glob '!docs/design.md'

# Full CI check (mirrors the `ci` GitHub workflow)
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
  cargo install cargo-tarpaulin
  cargo install typos-cli
  cargo install rumdl
  cargo install cargo-audit
  cargo install cargo-outdated
  cargo install cargo-nextest
  rustup component add miri --toolchain nightly

# Generate documentation for the workspace
docs:
  cargo doc --no-deps --open

# Run security audit
audit:
  cargo audit

# Check for outdated dependencies
outdated:
  cargo outdated

# Run clippy with pedantic, nursery and the cargo lint groups
clippy-all:
  RUSTC_WRAPPER= cargo +nightly clippy --all --all-targets --all-features -- -W clippy::pedantic -W clippy::nursery -W clippy::cargo

# Run tests with nextest
test-nextest:
  cargo nextest run --all-features

# Run miri on unsafe code
miri:
  cargo +nightly miri test --all-features

# Check for unused dependencies
unused-deps:
  cargo shear --check

# Update all dependencies
update-deps:
  cargo update

# Full check (lint + test + build + audit)
full-check: lint test build audit
