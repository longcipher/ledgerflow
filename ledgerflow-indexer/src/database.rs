use eyre::Result;
use sqlx::PgPool;

use crate::types::{ChainState, DepositEvent};

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Database { pool })
    }

    pub async fn get_chain_state(
        &self,
        chain_id: u64,
        contract_address: &str,
    ) -> Result<Option<ChainState>> {
        let result = sqlx::query_as::<_, ChainState>(
            "SELECT chain_id, contract_address, last_scanned_block, updated_at FROM chain_states WHERE chain_id = $1 AND contract_address = $2"
        )
        .bind(chain_id as i64)
        .bind(contract_address)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn update_chain_state(
        &self,
        chain_id: u64,
        contract_address: &str,
        block_number: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO chain_states (chain_id, contract_address, last_scanned_block, updated_at) 
             VALUES ($1, $2, $3, NOW()) 
             ON CONFLICT (chain_id, contract_address) 
             DO UPDATE SET last_scanned_block = $3, updated_at = NOW()",
        )
        .bind(chain_id as i64)
        .bind(contract_address)
        .bind(block_number)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_deposit_event(&self, event: &DepositEvent) -> Result<()> {
        sqlx::query(
            "INSERT INTO deposit_events (chain_id, contract_address, order_id, sender, amount, transaction_hash, block_number, log_index, created_at, processed) 
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), false)
             ON CONFLICT (chain_id, transaction_hash, log_index) DO NOTHING"
        )
        .bind(event.chain_id)
        .bind(&event.contract_address)
        .bind(&event.order_id)
        .bind(&event.sender)
        .bind(&event.amount)
        .bind(&event.transaction_hash)
        .bind(event.block_number)
        .bind(event.log_index)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn mark_order_completed(&self, order_id: &str) -> Result<()> {
        sqlx::query("UPDATE deposit_events SET processed = true WHERE order_id = $1")
            .bind(order_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_unprocessed_events(&self, chain_id: u64) -> Result<Vec<DepositEvent>> {
        let events = sqlx::query_as::<_, DepositEvent>(
            "SELECT id, chain_id, contract_address, order_id, sender, amount, transaction_hash, block_number, log_index, created_at, processed 
             FROM deposit_events 
             WHERE chain_id = $1 AND processed = false 
             ORDER BY block_number, log_index"
        )
        .bind(chain_id as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Update order status to 'deposited' when deposit event is detected
    pub async fn update_order_status_deposited(
        &self,
        order_id: &str,
        transaction_hash: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE orders SET status = 'deposited', transaction_hash = $1, updated_at = NOW() WHERE order_id = $2"
        )
        .bind(transaction_hash)
        .bind(order_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
