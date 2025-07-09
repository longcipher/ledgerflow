-- Create chain_states table to track scanning progress
CREATE TABLE IF NOT EXISTS chain_states (
    chain_id INTEGER NOT NULL,
    contract_address VARCHAR(255) NOT NULL,
    last_scanned_block BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    PRIMARY KEY (chain_id, contract_address)
);

-- Create deposit_events table to store parsed events
CREATE TABLE IF NOT EXISTS deposit_events (
    id BIGSERIAL PRIMARY KEY,
    chain_id INTEGER NOT NULL,
    contract_address VARCHAR(255) NOT NULL,
    order_id VARCHAR(255) NOT NULL,
    sender VARCHAR(255) NOT NULL,
    amount VARCHAR(255) NOT NULL,
    transaction_hash VARCHAR(255) NOT NULL,
    block_number BIGINT NOT NULL,
    log_index BIGINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    processed BOOLEAN NOT NULL DEFAULT false,
    UNIQUE (chain_id, transaction_hash, log_index)
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_deposit_events_chain_id ON deposit_events (chain_id);
CREATE INDEX IF NOT EXISTS idx_deposit_events_order_id ON deposit_events (order_id);
CREATE INDEX IF NOT EXISTS idx_deposit_events_processed ON deposit_events (processed);
CREATE INDEX IF NOT EXISTS idx_deposit_events_block_number ON deposit_events (block_number);
CREATE INDEX IF NOT EXISTS idx_chain_states_chain_contract ON chain_states (chain_id, contract_address);
