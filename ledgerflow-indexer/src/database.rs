use eyre::Result;
use sqlx::PgPool;
use tracing::{debug, info, warn};

use crate::types::{ChainState, DepositEvent};

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to database: {}", database_url);
        let pool = PgPool::connect(database_url).await?;
        info!("Successfully connected to database");
        Ok(Database { pool })
    }

    pub async fn get_chain_state(
        &self,
        chain_id: u64,
        contract_address: &str,
    ) -> Result<Option<ChainState>> {
        debug!(
            "Getting chain state for chain_id: {}, contract: {}",
            chain_id, contract_address
        );

        let result = sqlx::query_as::<_, ChainState>(
            "SELECT chain_id, contract_address, last_scanned_block, updated_at FROM chain_states WHERE chain_id = $1 AND contract_address = $2",
        )
        .bind(chain_id as i64)
        .bind(contract_address)
        .fetch_optional(&self.pool)
        .await?;

        match &result {
            Some(state) => info!(
                "Found chain state for chain {}: last_scanned_block = {}",
                chain_id, state.last_scanned_block
            ),
            None => info!(
                "No chain state found for chain {}, will start from configured start_block",
                chain_id
            ),
        }

        Ok(result)
    }

    pub async fn update_chain_state(
        &self,
        chain_id: u64,
        contract_address: &str,
        block_number: i64,
    ) -> Result<()> {
        debug!(
            "Updating chain state for chain_id: {}, contract: {}, block: {}",
            chain_id, contract_address, block_number
        );

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

        info!(
            "Updated chain state for chain {} to block {}",
            chain_id, block_number
        );
        Ok(())
    }

    pub async fn insert_deposit_event(&self, event: &DepositEvent) -> Result<()> {
        debug!(
            "Inserting deposit event: order_id={}, chain_id={}, block={}, tx_hash={}",
            event.order_id, event.chain_id, event.block_number, event.transaction_hash
        );

        let result = sqlx::query(
            "INSERT INTO deposit_events (chain_id, contract_address, order_id, sender, amount, transaction_hash, block_number, log_index, created_at, processed) 
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), false)
             ON CONFLICT (chain_id, transaction_hash, log_index) DO NOTHING",
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

        if result.rows_affected() > 0 {
            info!(
                "Inserted new deposit event for order_id: {}",
                event.order_id
            );
        } else {
            warn!(
                "Deposit event already exists for order_id: {} (duplicate)",
                event.order_id
            );
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn mark_order_completed(&self, order_id: &str) -> Result<()> {
        debug!("Marking order completed: {}", order_id);

        sqlx::query("UPDATE deposit_events SET processed = true WHERE order_id = $1")
            .bind(order_id)
            .execute(&self.pool)
            .await?;

        info!("Marked order {} as completed", order_id);
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_unprocessed_events(&self, chain_id: u64) -> Result<Vec<DepositEvent>> {
        debug!("Getting unprocessed events for chain_id: {}", chain_id);

        let events = sqlx::query_as::<_, DepositEvent>(
            "SELECT id, chain_id, contract_address, order_id, sender, amount, transaction_hash, block_number, log_index, created_at, processed 
             FROM deposit_events 
             WHERE chain_id = $1 AND processed = false 
             ORDER BY block_number, log_index",
        )
        .bind(chain_id as i64)
        .fetch_all(&self.pool)
        .await?;

        info!(
            "Found {} unprocessed events for chain {}",
            events.len(),
            chain_id
        );
        Ok(events)
    }

    pub async fn update_order_with_deposit_details(
        &self,
        order_id: &str,
        transaction_hash: &str,
        amount: &str,
        chain_id: i64,
    ) -> Result<()> {
        debug!(
            "Updating order with deposit details - order_id: {}, tx_hash: {}, amount: {}, chain_id: {}",
            order_id, transaction_hash, amount, chain_id
        );

        let result = sqlx::query(
            "UPDATE orders SET 
                status = 'deposited', 
                transaction_hash = $1, 
                amount = $2, 
                chain_id = $3, 
                updated_at = NOW() 
             WHERE order_id = $4",
        )
        .bind(transaction_hash)
        .bind(amount)
        .bind(chain_id)
        .bind(order_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() > 0 {
            info!(
                "Updated order {} with deposit details - status: deposited, amount: {}, chain_id: {}",
                order_id, amount, chain_id
            );
        } else {
            warn!(
                "Order {} not found in database or already updated",
                order_id
            );
        }

        Ok(())
    }
}
