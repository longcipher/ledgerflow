use std::{sync::Arc, time::Duration};

use eyre::{Context, Result};
use sui_json_rpc_types::{SuiEvent, SuiTransactionBlockResponse};
use sui_sdk::{SuiClient, SuiClientBuilder};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

use crate::{
    config::Config,
    database::Database,
    health,
    models::{SuiDepositEventData, SuiOwnershipTransferEventData, SuiWithdrawEventData},
};

pub struct SuiIndexer {
    client: SuiClient,
    database: Arc<Database>,
    config: Config,
    chain_id: String,
}

impl SuiIndexer {
    /// Create a new Sui indexer instance
    pub async fn new(config: Config) -> Result<Self> {
        info!("🔗 Connecting to Sui network: {}", config.network.rpc_url);

        let client = SuiClientBuilder::default()
            .build(&config.network.rpc_url)
            .await
            .wrap_err("Failed to create Sui client")?;

        // Verify connection and get chain identifier
        let chain_id = client
            .read_api()
            .get_chain_identifier()
            .await
            .wrap_err("Failed to get chain identifier")?;

        info!("✅ Connected to Sui network with chain ID: {}", chain_id);

        let database = Arc::new(
            Database::new(
                &config.database.connection_string,
                config.database.max_connections,
            )
            .await
            .wrap_err("Failed to connect to database")?,
        );

        Ok(SuiIndexer {
            client,
            database,
            config,
            chain_id,
        })
    }

    /// Start the indexer process
    pub async fn start(&self) -> Result<()> {
        info!(
            "🚀 Starting Sui indexer for package: {}",
            self.config.contract.package_id
        );

        // Start health check server in background
        let health_database = Arc::clone(&self.database);
        let health_port = self.config.health_check_port;
        tokio::spawn(async move {
            if let Err(e) = health::start_health_server(health_port, health_database).await {
                error!("Health server error: {:?}", e);
            }
        });

        // Get starting checkpoint
        let starting_checkpoint = self.get_starting_checkpoint().await?;
        info!(
            "📍 Starting indexing from checkpoint: {}",
            starting_checkpoint
        );

        // Start the main indexing loop
        self.index_events(starting_checkpoint).await
    }

    /// Get the starting checkpoint for indexing
    async fn get_starting_checkpoint(&self) -> Result<u64> {
        // Check if we have previous state
        if let Some(state) = self
            .database
            .get_indexer_state(&self.chain_id, &self.config.contract.package_id)
            .await?
        {
            info!(
                "📋 Resuming from previous checkpoint: {}",
                state.last_processed_checkpoint
            );
            Ok(state.last_processed_checkpoint as u64 + 1) // Start from next checkpoint
        } else {
            info!(
                "🆕 Starting fresh from configured checkpoint: {}",
                self.config.indexer.starting_checkpoint
            );
            Ok(self.config.indexer.starting_checkpoint)
        }
    }

    /// Main indexing loop that processes checkpoints
    async fn index_events(&self, starting_checkpoint: u64) -> Result<()> {
        let mut current_checkpoint = starting_checkpoint;
        let mut consecutive_errors = 0;
        const MAX_CONSECUTIVE_ERRORS: u32 = 10;

        loop {
            match self.process_checkpoint_batch(current_checkpoint).await {
                Ok(processed_count) => {
                    consecutive_errors = 0; // Reset error counter on success

                    if processed_count > 0 {
                        current_checkpoint += processed_count;
                        info!(
                            "✅ Processed {} checkpoints, now at checkpoint: {}",
                            processed_count, current_checkpoint
                        );
                    } else {
                        // No new checkpoints available, wait before checking again
                        debug!("⏳ No new checkpoints available, waiting...");
                        sleep(Duration::from_millis(
                            self.config.indexer.processing_delay_ms,
                        ))
                        .await;
                    }
                }
                Err(e) => {
                    consecutive_errors += 1;
                    warn!(
                        "❌ Error processing checkpoint {}: {:?} (consecutive errors: {})",
                        current_checkpoint, e, consecutive_errors
                    );

                    if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                        error!(
                            "💥 Too many consecutive errors ({}), stopping indexer",
                            consecutive_errors
                        );
                        return Err(e);
                    }

                    // Exponential backoff for errors
                    let delay = self.config.indexer.retry_delay_ms * (consecutive_errors as u64);
                    warn!("⏱️  Retrying in {} ms...", delay);
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }

    /// Process a batch of checkpoints
    async fn process_checkpoint_batch(&self, starting_checkpoint: u64) -> Result<u64> {
        // Get the latest checkpoint
        let latest_checkpoint = self
            .client
            .read_api()
            .get_latest_checkpoint_sequence_number()
            .await
            .wrap_err("Failed to get latest checkpoint")?;

        if starting_checkpoint > latest_checkpoint {
            // We're caught up, return 0 to indicate no processing needed
            return Ok(0);
        }

        // Calculate how many checkpoints to process in this batch
        let end_checkpoint = std::cmp::min(
            starting_checkpoint + self.config.indexer.checkpoint_batch_size - 1,
            latest_checkpoint,
        );

        debug!(
            "🔍 Processing checkpoint range: {} to {}",
            starting_checkpoint, end_checkpoint
        );

        let mut processed_count = 0;

        for checkpoint_seq in starting_checkpoint..=end_checkpoint {
            match self.process_single_checkpoint(checkpoint_seq).await {
                Ok(()) => {
                    processed_count += 1;

                    // Update indexer state in database
                    self.database
                        .upsert_indexer_state(
                            &self.chain_id,
                            &self.config.contract.package_id,
                            checkpoint_seq as i64,
                            None,
                            "active",
                        )
                        .await
                        .wrap_err("Failed to update indexer state")?;
                }
                Err(e) => {
                    error!(
                        "💥 Failed to process checkpoint {}: {:?}",
                        checkpoint_seq, e
                    );
                    return Err(e);
                }
            }

            // Small delay between checkpoints to avoid overwhelming the node
            if processed_count % 10 == 0 && processed_count > 0 {
                sleep(Duration::from_millis(100)).await;
            }
        }

        Ok(processed_count)
    }

    /// Process a single checkpoint and extract relevant events
    async fn process_single_checkpoint(&self, checkpoint_seq: u64) -> Result<()> {
        debug!("🔍 Processing checkpoint: {}", checkpoint_seq);

        // Get checkpoint data with transaction details
        let checkpoint = timeout(
            Duration::from_secs(30),
            self.client.read_api().get_checkpoint(checkpoint_seq.into()),
        )
        .await
        .wrap_err("Timeout getting checkpoint data")?
        .wrap_err("Failed to get checkpoint data")?;

        if checkpoint.transactions.is_empty() {
            debug!("📭 No transactions in checkpoint {}", checkpoint_seq);
            return Ok(());
        }

        debug!(
            "📦 Processing {} transactions in checkpoint {}",
            checkpoint.transactions.len(),
            checkpoint_seq
        );

        // Process each transaction in the checkpoint
        for (tx_index, tx_digest) in checkpoint.transactions.iter().enumerate() {
            if let Err(e) = self
                .process_transaction(&tx_digest.to_string(), checkpoint_seq, tx_index)
                .await
            {
                warn!(
                    "⚠️  Failed to process transaction {} in checkpoint {}: {:?}",
                    tx_digest, checkpoint_seq, e
                );
                // Continue processing other transactions instead of failing the entire checkpoint
            }
        }

        Ok(())
    }

    /// Process a single transaction and extract events
    async fn process_transaction(
        &self,
        tx_digest: &str,
        checkpoint_seq: u64,
        tx_index: usize,
    ) -> Result<()> {
        debug!(
            "🔍 Processing transaction: {} (checkpoint: {}, index: {})",
            tx_digest, checkpoint_seq, tx_index
        );

        // Get transaction details with events
        let tx_response: SuiTransactionBlockResponse = timeout(
            Duration::from_secs(10),
            self.client.read_api().get_transaction_with_options(
                tx_digest
                    .parse()
                    .map_err(|e| eyre::eyre!("Failed to parse transaction digest: {}", e))?,
                sui_json_rpc_types::SuiTransactionBlockResponseOptions::new()
                    .with_events()
                    .with_input()
                    .with_effects(),
            ),
        )
        .await
        .wrap_err("Timeout getting transaction details")?
        .wrap_err("Failed to get transaction details")?;

        // Check if the transaction has events
        let events = match tx_response.events {
            Some(events) => events,
            None => {
                debug!("📭 No events in transaction {}", tx_digest);
                return Ok(());
            }
        };

        if events.data.is_empty() {
            debug!("📭 No event data in transaction {}", tx_digest);
            return Ok(());
        }

        debug!(
            "📋 Found {} events in transaction {}",
            events.data.len(),
            tx_digest
        );

        // Process each event
        for (event_index, event) in events.data.iter().enumerate() {
            if let Err(e) = self
                .process_event(event, checkpoint_seq, tx_digest, event_index as i32)
                .await
            {
                warn!(
                    "⚠️  Failed to process event {} in transaction {}: {:?}",
                    event_index, tx_digest, e
                );
                // Continue processing other events
            }
        }

        Ok(())
    }

    /// Process a single event and store relevant data
    async fn process_event(
        &self,
        event: &SuiEvent,
        checkpoint_seq: u64,
        tx_digest: &str,
        event_index: i32,
    ) -> Result<()> {
        // Check if this event is from our target package
        if !event
            .package_id
            .to_string()
            .eq(&self.config.contract.package_id)
        {
            debug!(
                "🚫 Skipping event from different package: {}",
                event.package_id
            );
            return Ok(());
        }

        // Check if this event is from our target module
        if event.transaction_module.as_str() != self.config.contract.module_name.as_str() {
            debug!(
                "🚫 Skipping event from different module: {}",
                event.transaction_module
            );
            return Ok(());
        }

        debug!(
            "🎯 Processing event type: {} from package: {}",
            event.type_, event.package_id
        );

        // Check if we've already processed this event
        if self
            .database
            .event_exists(
                &self.chain_id,
                &self.config.contract.package_id,
                tx_digest,
                event_index,
            )
            .await?
        {
            debug!("⏭️  Event already processed, skipping");
            return Ok(());
        }

        // Process based on event type
        if event.type_.name.as_str() == self.config.contract.deposit_event_type {
            self.process_deposit_event(event, checkpoint_seq, tx_digest, event_index)
                .await?;
        } else if event.type_.name.as_str() == self.config.contract.withdraw_event_type {
            self.process_withdraw_event(event, checkpoint_seq, tx_digest, event_index)
                .await?;
        } else if event.type_.name.as_str() == self.config.contract.ownership_transfer_event_type {
            self.process_ownership_transfer_event(event, checkpoint_seq, tx_digest, event_index)
                .await?;
        } else {
            debug!("🚫 Ignoring unknown event type: {}", event.type_.name);
        }

        Ok(())
    }

    /// Process a deposit event
    async fn process_deposit_event(
        &self,
        event: &SuiEvent,
        checkpoint_seq: u64,
        tx_digest: &str,
        event_index: i32,
    ) -> Result<()> {
        debug!("💰 Processing deposit event");

        // Parse the event data from parsed_json instead of bcs
        let event_json = event
            .parsed_json
            .as_object()
            .ok_or_else(|| eyre::eyre!("Event parsed_json is not an object"))?;

        let vault_id = event_json
            .get("vault_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Missing vault_id in event"))?;

        let payer = event_json
            .get("payer")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Missing payer in event"))?;

        let order_id_array = event_json
            .get("order_id")
            .and_then(|v| v.as_array())
            .ok_or_else(|| eyre::eyre!("Missing order_id in event"))?;

        let order_id: Vec<u8> = order_id_array
            .iter()
            .map(|v| v.as_u64().unwrap_or(0) as u8)
            .collect();

        let amount = event_json
            .get("amount")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| eyre::eyre!("Missing or invalid amount in event"))?;

        let timestamp = event_json
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| eyre::eyre!("Missing or invalid timestamp in event"))?;

        let deposit_index = event_json
            .get("deposit_index")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| eyre::eyre!("Missing or invalid deposit_index in event"))?;

        let event_data = SuiDepositEventData {
            vault_id: vault_id.to_string(),
            payer: payer.to_string(),
            order_id,
            amount,
            timestamp,
            deposit_index,
        };

        info!(
            "💰 Deposit event: vault={}, payer={}, amount={}, order_id={}",
            event_data.vault_id,
            event_data.payer,
            event_data.amount,
            hex::encode(&event_data.order_id)
        );

        // Store in database
        self.database
            .insert_deposit_event(
                &self.chain_id,
                &self.config.contract.package_id,
                &event_data,
                checkpoint_seq as i64,
                tx_digest,
                event_index,
            )
            .await
            .wrap_err("Failed to insert deposit event")?;

        Ok(())
    }

    /// Process a withdrawal event
    async fn process_withdraw_event(
        &self,
        event: &SuiEvent,
        checkpoint_seq: u64,
        tx_digest: &str,
        event_index: i32,
    ) -> Result<()> {
        debug!("💸 Processing withdraw event");

        // Parse the event data from parsed_json
        let event_json = event
            .parsed_json
            .as_object()
            .ok_or_else(|| eyre::eyre!("Event parsed_json is not an object"))?;

        let vault_id = event_json
            .get("vault_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Missing vault_id in event"))?;

        let owner = event_json
            .get("owner")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Missing owner in event"))?;

        let recipient = event_json
            .get("recipient")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Missing recipient in event"))?;

        let amount = event_json
            .get("amount")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| eyre::eyre!("Missing or invalid amount in event"))?;

        let timestamp = event_json
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| eyre::eyre!("Missing or invalid timestamp in event"))?;

        let event_data = SuiWithdrawEventData {
            vault_id: vault_id.to_string(),
            owner: owner.to_string(),
            recipient: recipient.to_string(),
            amount,
            timestamp,
        };

        info!(
            "💸 Withdraw event: vault={}, owner={}, recipient={}, amount={}",
            event_data.vault_id, event_data.owner, event_data.recipient, event_data.amount
        );

        // Store in database
        self.database
            .insert_withdraw_event(
                &self.chain_id,
                &self.config.contract.package_id,
                &event_data,
                checkpoint_seq as i64,
                tx_digest,
                event_index,
            )
            .await
            .wrap_err("Failed to insert withdraw event")?;

        Ok(())
    }

    /// Process an ownership transfer event
    async fn process_ownership_transfer_event(
        &self,
        event: &SuiEvent,
        checkpoint_seq: u64,
        tx_digest: &str,
        event_index: i32,
    ) -> Result<()> {
        debug!("👑 Processing ownership transfer event");

        // Parse the event data from parsed_json
        let event_json = event
            .parsed_json
            .as_object()
            .ok_or_else(|| eyre::eyre!("Event parsed_json is not an object"))?;

        let vault_id = event_json
            .get("vault_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Missing vault_id in event"))?;

        let previous_owner = event_json
            .get("previous_owner")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Missing previous_owner in event"))?;

        let new_owner = event_json
            .get("new_owner")
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Missing new_owner in event"))?;

        let timestamp = event_json
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| eyre::eyre!("Missing or invalid timestamp in event"))?;

        let event_data = SuiOwnershipTransferEventData {
            vault_id: vault_id.to_string(),
            previous_owner: previous_owner.to_string(),
            new_owner: new_owner.to_string(),
            timestamp,
        };

        info!(
            "👑 Ownership transfer event: vault={}, previous_owner={}, new_owner={}",
            event_data.vault_id, event_data.previous_owner, event_data.new_owner
        );

        // Store in database
        self.database
            .insert_ownership_transfer_event(
                &self.chain_id,
                &self.config.contract.package_id,
                &event_data,
                checkpoint_seq as i64,
                tx_digest,
                event_index,
            )
            .await
            .wrap_err("Failed to insert ownership transfer event")?;

        Ok(())
    }
}

/// Main entry point for running the indexer
pub async fn run_indexer(config: Config) -> Result<()> {
    let indexer = SuiIndexer::new(config).await?;
    indexer.start().await
}
