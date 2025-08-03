-- Create tables for Sui indexer state and events

-- Indexer state tracking table
CREATE TABLE IF NOT EXISTS sui_indexer_state (
    id SERIAL PRIMARY KEY,
    chain_id VARCHAR(64) NOT NULL,
    package_id VARCHAR(64) NOT NULL,
    last_processed_checkpoint BIGINT NOT NULL DEFAULT 0,
    last_processed_transaction VARCHAR(255),
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (chain_id, package_id)
);

-- Index for efficient lookups
CREATE INDEX IF NOT EXISTS idx_sui_indexer_state_chain_package
ON sui_indexer_state (chain_id, package_id);

-- Deposit events table
CREATE TABLE IF NOT EXISTS sui_deposit_events (
    id SERIAL PRIMARY KEY,
    chain_id VARCHAR(64) NOT NULL,
    package_id VARCHAR(64) NOT NULL,
    vault_id VARCHAR(64) NOT NULL,
    payer VARCHAR(64) NOT NULL,
    order_id VARCHAR(255) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL, -- Support arbitrary precision for large amounts
    timestamp BIGINT NOT NULL, -- Unix timestamp in milliseconds
    deposit_index BIGINT NOT NULL,
    checkpoint_sequence BIGINT NOT NULL,
    transaction_digest VARCHAR(255) NOT NULL,
    event_index INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (chain_id, package_id, transaction_digest, event_index)
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_sui_deposit_events_vault
ON sui_deposit_events (chain_id, package_id, vault_id);

CREATE INDEX IF NOT EXISTS idx_sui_deposit_events_payer
ON sui_deposit_events (chain_id, package_id, payer);

CREATE INDEX IF NOT EXISTS idx_sui_deposit_events_order_id
ON sui_deposit_events (chain_id, package_id, order_id);

CREATE INDEX IF NOT EXISTS idx_sui_deposit_events_checkpoint
ON sui_deposit_events (checkpoint_sequence);

-- Withdraw events table
CREATE TABLE IF NOT EXISTS sui_withdraw_events (
    id SERIAL PRIMARY KEY,
    chain_id VARCHAR(64) NOT NULL,
    package_id VARCHAR(64) NOT NULL,
    vault_id VARCHAR(64) NOT NULL,
    owner VARCHAR(64) NOT NULL,
    recipient VARCHAR(64) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    timestamp BIGINT NOT NULL,
    checkpoint_sequence BIGINT NOT NULL,
    transaction_digest VARCHAR(255) NOT NULL,
    event_index INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (chain_id, package_id, transaction_digest, event_index)
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_sui_withdraw_events_vault
ON sui_withdraw_events (chain_id, package_id, vault_id);

CREATE INDEX IF NOT EXISTS idx_sui_withdraw_events_owner
ON sui_withdraw_events (chain_id, package_id, owner);

CREATE INDEX IF NOT EXISTS idx_sui_withdraw_events_recipient
ON sui_withdraw_events (chain_id, package_id, recipient);

CREATE INDEX IF NOT EXISTS idx_sui_withdraw_events_checkpoint
ON sui_withdraw_events (checkpoint_sequence);

-- Ownership transfer events table
CREATE TABLE IF NOT EXISTS sui_ownership_transfer_events (
    id SERIAL PRIMARY KEY,
    chain_id VARCHAR(64) NOT NULL,
    package_id VARCHAR(64) NOT NULL,
    vault_id VARCHAR(64) NOT NULL,
    previous_owner VARCHAR(64) NOT NULL,
    new_owner VARCHAR(64) NOT NULL,
    timestamp BIGINT NOT NULL,
    checkpoint_sequence BIGINT NOT NULL,
    transaction_digest VARCHAR(255) NOT NULL,
    event_index INTEGER NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE (chain_id, package_id, transaction_digest, event_index)
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_sui_ownership_transfer_events_vault
ON sui_ownership_transfer_events (chain_id, package_id, vault_id);

CREATE INDEX IF NOT EXISTS idx_sui_ownership_transfer_events_previous_owner
ON sui_ownership_transfer_events (chain_id, package_id, previous_owner);

CREATE INDEX IF NOT EXISTS idx_sui_ownership_transfer_events_new_owner
ON sui_ownership_transfer_events (chain_id, package_id, new_owner);

CREATE INDEX IF NOT EXISTS idx_sui_ownership_transfer_events_checkpoint
ON sui_ownership_transfer_events (checkpoint_sequence);

-- Add trigger to automatically update updated_at timestamp for indexer state
CREATE OR REPLACE FUNCTION update_sui_indexer_state_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER sui_indexer_state_updated_at_trigger
    BEFORE UPDATE ON sui_indexer_state
    FOR EACH ROW
    EXECUTE FUNCTION update_sui_indexer_state_updated_at();
