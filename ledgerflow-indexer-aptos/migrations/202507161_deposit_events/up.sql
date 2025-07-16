CREATE TABLE IF NOT EXISTS aptos_deposit_events (
    id SERIAL PRIMARY KEY,
    chain_id VARCHAR(64) NOT NULL,
    contract_address VARCHAR(64) NOT NULL,
    payer VARCHAR(64) NOT NULL,
    order_id VARCHAR(255) NOT NULL,
    amount NUMERIC(78, 0) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    deposit_index BIGINT NOT NULL,
    txn_version BIGINT NOT NULL,
    event_index INT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(chain_id, contract_address, txn_version, event_index)
);
