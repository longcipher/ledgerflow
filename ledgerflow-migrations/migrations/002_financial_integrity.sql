-- Strengthen financial integrity and authentication schema for production use.

BEGIN;

-- Ensure account API token hash storage exists for balancer auth.
ALTER TABLE accounts
    ADD COLUMN IF NOT EXISTS api_token_hash VARCHAR(64);

CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_api_token_hash_unique
ON accounts (api_token_hash)
WHERE api_token_hash IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_accounts_email_unique
ON accounts (email)
WHERE email IS NOT NULL;

-- Guardrails before converting amount/balance columns to NUMERIC.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM orders WHERE amount !~ '^[0-9]+$') THEN
        RAISE EXCEPTION 'orders.amount must be unsigned integer strings before migration 002';
    END IF;

    IF EXISTS (SELECT 1 FROM balances WHERE balance !~ '^[0-9]+$') THEN
        RAISE EXCEPTION 'balances.balance must be unsigned integer strings before migration 002';
    END IF;

    IF EXISTS (SELECT 1 FROM deposit_events WHERE amount !~ '^[0-9]+$') THEN
        RAISE EXCEPTION 'deposit_events.amount must be unsigned integer strings before migration 002';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM orders o
        LEFT JOIN accounts a ON a.id = o.account_id
        WHERE a.id IS NULL
    ) THEN
        RAISE EXCEPTION 'orders contains orphan account_id rows; fix data before migration 002';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM balances b
        LEFT JOIN accounts a ON a.id = b.account_id
        WHERE a.id IS NULL
    ) THEN
        RAISE EXCEPTION 'balances contains orphan account_id rows; fix data before migration 002';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM deposit_events d
        LEFT JOIN orders o ON o.order_id = d.order_id
        WHERE o.order_id IS NULL
    ) THEN
        RAISE EXCEPTION 'deposit_events contains orphan order_id rows; fix data before migration 002';
    END IF;
END
$$;

ALTER TABLE orders
    ALTER COLUMN amount TYPE NUMERIC(78,0) USING amount::NUMERIC(78,0),
    ADD CONSTRAINT orders_amount_positive_chk CHECK (amount > 0),
    ADD CONSTRAINT orders_chain_id_positive_chk CHECK (chain_id > 0),
    ADD CONSTRAINT orders_account_id_fk
        FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE RESTRICT;

ALTER TABLE balances
    ALTER COLUMN balance TYPE NUMERIC(78,0) USING balance::NUMERIC(78,0),
    ADD CONSTRAINT balances_balance_non_negative_chk CHECK (balance >= 0),
    ADD CONSTRAINT balances_account_id_fk
        FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE;

ALTER TABLE deposit_events
    ALTER COLUMN amount TYPE NUMERIC(78,0) USING amount::NUMERIC(78,0),
    ADD CONSTRAINT deposit_events_amount_positive_chk CHECK (amount > 0),
    ADD CONSTRAINT deposit_events_order_id_fk
        FOREIGN KEY (order_id) REFERENCES orders(order_id) ON DELETE CASCADE;

COMMIT;
