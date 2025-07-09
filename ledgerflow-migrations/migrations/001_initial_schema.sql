-- Initial schema for LedgerFlow system
-- This migration combines all the necessary tables for the entire system

-- Create enum for order status
CREATE TYPE order_status AS ENUM ('pending', 'deposited', 'completed', 'failed', 'cancelled');

-- Create accounts table (from ledgerflow-balancer)
CREATE TABLE IF NOT EXISTS accounts (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(255) NOT NULL UNIQUE,
    telegram_id BIGINT NOT NULL UNIQUE,
    email VARCHAR(320),
    evm_address VARCHAR(42),
    encrypted_pk TEXT,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create orders table (unified from all services)
CREATE TABLE IF NOT EXISTS orders (
    id BIGSERIAL PRIMARY KEY,
    order_id VARCHAR(255) NOT NULL UNIQUE,
    account_id BIGINT NOT NULL,
    broker_id VARCHAR(255) NOT NULL,
    amount VARCHAR(255) NOT NULL,
    token_address VARCHAR(42) NOT NULL,
    chain_id BIGINT NOT NULL DEFAULT 1,
    status order_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    transaction_hash VARCHAR(66),
    notified BOOLEAN NOT NULL DEFAULT FALSE
);

-- Create balances table (for tracking user balance aggregations)
CREATE TABLE IF NOT EXISTS balances (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT NOT NULL UNIQUE,
    balance VARCHAR(255) NOT NULL DEFAULT '0',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create chain_states table (from ledgerflow-indexer)
CREATE TABLE IF NOT EXISTS chain_states (
    chain_id BIGINT NOT NULL,
    contract_address VARCHAR(255) NOT NULL,
    last_scanned_block BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    PRIMARY KEY (chain_id, contract_address)
);

-- Create deposit_events table (from ledgerflow-indexer)
CREATE TABLE IF NOT EXISTS deposit_events (
    id BIGSERIAL PRIMARY KEY,
    chain_id BIGINT NOT NULL,
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

-- orders table indexes
CREATE INDEX IF NOT EXISTS idx_orders_account_id ON orders (account_id) ;
CREATE INDEX IF NOT EXISTS idx_orders_chain_id ON orders (chain_id) ;
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders (status) ;
CREATE INDEX IF NOT EXISTS idx_orders_account_status ON orders (account_id, status);

-- deposit_events table indexes
CREATE INDEX IF NOT EXISTS idx_deposit_events_chain_id
ON deposit_events (chain_id) ;
CREATE INDEX IF NOT EXISTS idx_deposit_events_order_id
ON deposit_events (order_id) ;
CREATE INDEX IF NOT EXISTS idx_deposit_events_processed
ON deposit_events (processed) ;
CREATE INDEX IF NOT EXISTS idx_deposit_events_block_number
ON deposit_events (block_number) ;
CREATE INDEX IF NOT EXISTS idx_deposit_events_chain_processed ON deposit_events (chain_id, processed);
CREATE INDEX IF NOT EXISTS idx_deposit_events_order_processed ON deposit_events (order_id, processed);
