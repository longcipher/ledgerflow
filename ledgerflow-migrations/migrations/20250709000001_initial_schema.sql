-- Initial schema for LedgerFlow system
-- This migration combines all the necessary tables for the entire system

-- Create enum for order status
CREATE TYPE order_status AS ENUM ('pending', 'completed', 'failed', 'cancelled');

-- Create accounts table (from ledgerflow-balancer)
CREATE TABLE IF NOT EXISTS accounts (
    id BIGSERIAL PRIMARY KEY,
    account_id VARCHAR(255) NOT NULL UNIQUE,
    email VARCHAR(255),
    telegram_id VARCHAR(255),
    evm_address VARCHAR(42),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create users table (from ledgerflow-bot)
CREATE TABLE IF NOT EXISTS users (
    id BIGSERIAL PRIMARY KEY,
    telegram_id BIGINT NOT NULL UNIQUE,
    username VARCHAR(255),
    first_name VARCHAR(255),
    last_name VARCHAR(255),
    evm_address VARCHAR(42),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create orders table (unified from all services)
CREATE TABLE IF NOT EXISTS orders (
    id BIGSERIAL PRIMARY KEY,
    order_id VARCHAR(255) NOT NULL UNIQUE,
    account_id VARCHAR(255) NOT NULL,
    broker_id VARCHAR(255) NOT NULL,
    amount VARCHAR(255) NOT NULL,
    token_address VARCHAR(42) NOT NULL,
    chain_id BIGINT NOT NULL DEFAULT 1,
    status order_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    transaction_hash VARCHAR(66)
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

-- Create indexes for accounts table
CREATE INDEX IF NOT EXISTS idx_accounts_account_id ON accounts(account_id);
CREATE INDEX IF NOT EXISTS idx_accounts_telegram_id ON accounts(telegram_id);
CREATE INDEX IF NOT EXISTS idx_accounts_evm_address ON accounts(evm_address);

-- Create indexes for users table
CREATE INDEX IF NOT EXISTS idx_users_telegram_id ON users(telegram_id);
CREATE INDEX IF NOT EXISTS idx_users_evm_address ON users(evm_address);

-- Create indexes for orders table
CREATE INDEX IF NOT EXISTS idx_orders_order_id ON orders(order_id);
CREATE INDEX IF NOT EXISTS idx_orders_account_id ON orders(account_id);
CREATE INDEX IF NOT EXISTS idx_orders_chain_id ON orders(chain_id);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);
CREATE INDEX IF NOT EXISTS idx_orders_created_at ON orders(created_at);

-- Create indexes for chain_states table
CREATE INDEX IF NOT EXISTS idx_chain_states_chain_contract ON chain_states(chain_id, contract_address);

-- Create indexes for deposit_events table
CREATE INDEX IF NOT EXISTS idx_deposit_events_chain_id ON deposit_events(chain_id);
CREATE INDEX IF NOT EXISTS idx_deposit_events_order_id ON deposit_events(order_id);
CREATE INDEX IF NOT EXISTS idx_deposit_events_processed ON deposit_events(processed);
CREATE INDEX IF NOT EXISTS idx_deposit_events_block_number ON deposit_events(block_number);

-- Create updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE 'plpgsql';

-- Create triggers for updated_at
CREATE TRIGGER update_accounts_updated_at BEFORE UPDATE ON accounts 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_orders_updated_at BEFORE UPDATE ON orders 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_chain_states_updated_at BEFORE UPDATE ON chain_states 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
