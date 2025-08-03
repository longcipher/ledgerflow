use eyre::Result;
use sqlx::PgPool;
use tracing::{debug, info};

use crate::models::{
    IndexerState, SuiDepositEvent, SuiDepositEventData, SuiOwnershipTransferEventData,
    SuiWithdrawEventData,
};

pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Create a new database connection from connection string
    pub async fn new(connection_string: &str, max_connections: u32) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(connection_string)
            .await?;

        // Run migrations if needed
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Database { pool })
    }

    /// Get the current indexer state for a specific chain and package
    pub async fn get_indexer_state(
        &self,
        chain_id: &str,
        package_id: &str,
    ) -> Result<Option<IndexerState>> {
        let result = sqlx::query_as::<_, IndexerState>(
            "SELECT * FROM sui_indexer_state WHERE chain_id = $1 AND package_id = $2",
        )
        .bind(chain_id)
        .bind(package_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Update or create indexer state
    pub async fn upsert_indexer_state(
        &self,
        chain_id: &str,
        package_id: &str,
        last_processed_checkpoint: i64,
        last_processed_transaction: Option<&str>,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sui_indexer_state (chain_id, package_id, last_processed_checkpoint, last_processed_transaction, status)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (chain_id, package_id)
            DO UPDATE SET
                last_processed_checkpoint = EXCLUDED.last_processed_checkpoint,
                last_processed_transaction = EXCLUDED.last_processed_transaction,
                status = EXCLUDED.status,
                updated_at = NOW()
            "#
        )
        .bind(chain_id)
        .bind(package_id)
        .bind(last_processed_checkpoint)
        .bind(last_processed_transaction)
        .bind(status)
        .execute(&self.pool)
        .await?;

        debug!(
            chain_id = chain_id,
            package_id = package_id,
            checkpoint = last_processed_checkpoint,
            "Updated indexer state"
        );

        Ok(())
    }

    /// Insert a new deposit event
    pub async fn insert_deposit_event(
        &self,
        chain_id: &str,
        package_id: &str,
        event_data: &SuiDepositEventData,
        checkpoint_sequence: i64,
        transaction_digest: &str,
        event_index: i32,
    ) -> Result<()> {
        // Convert order_id bytes to hex string
        let order_id_hex = hex::encode(&event_data.order_id);

        sqlx::query(
            r#"
            INSERT INTO sui_deposit_events 
            (chain_id, package_id, vault_id, payer, order_id, amount, timestamp, deposit_index, 
             checkpoint_sequence, transaction_digest, event_index)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (chain_id, package_id, transaction_digest, event_index) DO NOTHING
            "#,
        )
        .bind(chain_id)
        .bind(package_id)
        .bind(&event_data.vault_id)
        .bind(&event_data.payer)
        .bind(order_id_hex)
        .bind(bigdecimal::BigDecimal::from(event_data.amount))
        .bind(event_data.timestamp as i64)
        .bind(event_data.deposit_index as i64)
        .bind(checkpoint_sequence)
        .bind(transaction_digest)
        .bind(event_index)
        .execute(&self.pool)
        .await?;

        info!(
            vault_id = event_data.vault_id,
            payer = event_data.payer,
            amount = event_data.amount,
            order_id = hex::encode(&event_data.order_id),
            "Inserted deposit event"
        );

        Ok(())
    }

    /// Insert a new withdrawal event
    pub async fn insert_withdraw_event(
        &self,
        chain_id: &str,
        package_id: &str,
        event_data: &SuiWithdrawEventData,
        checkpoint_sequence: i64,
        transaction_digest: &str,
        event_index: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sui_withdraw_events 
            (chain_id, package_id, vault_id, owner, recipient, amount, timestamp, 
             checkpoint_sequence, transaction_digest, event_index)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (chain_id, package_id, transaction_digest, event_index) DO NOTHING
            "#,
        )
        .bind(chain_id)
        .bind(package_id)
        .bind(&event_data.vault_id)
        .bind(&event_data.owner)
        .bind(&event_data.recipient)
        .bind(bigdecimal::BigDecimal::from(event_data.amount))
        .bind(event_data.timestamp as i64)
        .bind(checkpoint_sequence)
        .bind(transaction_digest)
        .bind(event_index)
        .execute(&self.pool)
        .await?;

        info!(
            vault_id = event_data.vault_id,
            owner = event_data.owner,
            recipient = event_data.recipient,
            amount = event_data.amount,
            "Inserted withdraw event"
        );

        Ok(())
    }

    /// Insert a new ownership transfer event
    pub async fn insert_ownership_transfer_event(
        &self,
        chain_id: &str,
        package_id: &str,
        event_data: &SuiOwnershipTransferEventData,
        checkpoint_sequence: i64,
        transaction_digest: &str,
        event_index: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sui_ownership_transfer_events 
            (chain_id, package_id, vault_id, previous_owner, new_owner, timestamp, 
             checkpoint_sequence, transaction_digest, event_index)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (chain_id, package_id, transaction_digest, event_index) DO NOTHING
            "#,
        )
        .bind(chain_id)
        .bind(package_id)
        .bind(&event_data.vault_id)
        .bind(&event_data.previous_owner)
        .bind(&event_data.new_owner)
        .bind(event_data.timestamp as i64)
        .bind(checkpoint_sequence)
        .bind(transaction_digest)
        .bind(event_index)
        .execute(&self.pool)
        .await?;

        info!(
            vault_id = event_data.vault_id,
            previous_owner = event_data.previous_owner,
            new_owner = event_data.new_owner,
            "Inserted ownership transfer event"
        );

        Ok(())
    }

    /// Check if a specific transaction and event index combination exists
    pub async fn event_exists(
        &self,
        chain_id: &str,
        package_id: &str,
        transaction_digest: &str,
        event_index: i32,
    ) -> Result<bool> {
        let exists = sqlx::query(
            r#"
            SELECT 1 FROM sui_deposit_events 
            WHERE chain_id = $1 AND package_id = $2 AND transaction_digest = $3 AND event_index = $4
            UNION ALL
            SELECT 1 FROM sui_withdraw_events 
            WHERE chain_id = $1 AND package_id = $2 AND transaction_digest = $3 AND event_index = $4
            UNION ALL
            SELECT 1 FROM sui_ownership_transfer_events 
            WHERE chain_id = $1 AND package_id = $2 AND transaction_digest = $3 AND event_index = $4
            LIMIT 1
            "#,
        )
        .bind(chain_id)
        .bind(package_id)
        .bind(transaction_digest)
        .bind(event_index)
        .fetch_optional(&self.pool)
        .await?;

        Ok(exists.is_some())
    }

    /// Get deposit events for a specific vault
    #[allow(dead_code)]
    pub async fn get_deposit_events_for_vault(
        &self,
        chain_id: &str,
        package_id: &str,
        vault_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SuiDepositEvent>> {
        let events = sqlx::query_as::<_, SuiDepositEvent>(
            r#"
            SELECT * FROM sui_deposit_events 
            WHERE chain_id = $1 AND package_id = $2 AND vault_id = $3
            ORDER BY checkpoint_sequence DESC, event_index DESC
            LIMIT $4 OFFSET $5
            "#,
        )
        .bind(chain_id)
        .bind(package_id)
        .bind(vault_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    /// Get the health status of the database connection
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await?;
        Ok(())
    }
}
