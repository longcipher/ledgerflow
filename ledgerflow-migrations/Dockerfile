FROM rust:1.75 as builder

# Install sqlx-cli
RUN cargo install sqlx-cli --no-default-features --features postgres

# Set working directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY ledgerflow-migrations ./ledgerflow-migrations

# Build the migration crate
RUN cd ledgerflow-migrations && cargo build --release

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy sqlx-cli from builder
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/

# Copy migration files and scripts
COPY --from=builder /app/ledgerflow-migrations/migrations /app/migrations
COPY --from=builder /app/ledgerflow-migrations/migrate.sh /app/
COPY --from=builder /app/ledgerflow-migrations/target/release/ledgerflow-migrations /app/

# Set working directory
WORKDIR /app

# Make script executable
RUN chmod +x migrate.sh

# Set default command
CMD ["./migrate.sh", "migrate"]
