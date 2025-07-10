use std::sync::Arc;

use alloy::{
    network::Ethereum,
    primitives::{Address, FixedBytes, U256, keccak256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{Filter, Log},
};
use eyre::Result;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info};

use crate::{
    config::{ChainConfig, Config},
    database::Database,
    types::{DepositEvent, ParsedDepositEvent},
};

pub struct Indexer {
    config: Config,
    database: Arc<Database>,
}

impl Indexer {
    pub async fn new(config: Config, database: Database) -> Result<Self> {
        Ok(Indexer {
            config,
            database: Arc::new(database),
        })
    }

    pub async fn start(&self) -> Result<()> {
        let mut handles = Vec::new();

        info!("Starting indexer for {} chains", self.config.chains.len());

        // Start indexing for each chain
        for chain in &self.config.chains {
            let chain_config = chain.clone();
            let database = Arc::clone(&self.database);

            info!(
                "Spawning indexer task for chain: {} (chain_id: {})",
                chain_config.name, chain_config.chain_id
            );

            let handle = tokio::spawn(async move {
                if let Err(e) = Self::index_chain(chain_config, database).await {
                    error!("Error indexing chain: {}", e);
                }
            });

            handles.push(handle);
        }

        info!("All indexer tasks spawned, waiting for completion...");

        // Wait for all indexing tasks to complete
        for handle in handles {
            handle.await?;
        }

        info!("All indexer tasks completed");
        Ok(())
    }

    async fn index_chain(chain_config: ChainConfig, database: Arc<Database>) -> Result<()> {
        info!(
            "🚀 Starting indexer for chain: {} (chain_id: {})",
            chain_config.name, chain_config.chain_id
        );
        info!("📡 Connecting to RPC endpoint: {}", chain_config.rpc_http);

        // Create provider for this chain
        let provider = ProviderBuilder::new()
            .connect(&chain_config.rpc_http)
            .await?;

        info!("✅ Connected to RPC for chain: {}", chain_config.name);

        // Get the last scanned block from database
        let mut last_block = match database
            .get_chain_state(chain_config.chain_id, &chain_config.payment_vault_contract)
            .await?
        {
            Some(state) => state.last_scanned_block as u64,
            None => chain_config.start_block,
        };

        info!(
            "📊 Starting from block {} for chain {} (contract: {})",
            last_block, chain_config.name, chain_config.payment_vault_contract
        );

        // Contract address
        let contract_address: Address = chain_config.payment_vault_contract.parse()?;

        // DepositReceived event signature: DepositReceived(address,bytes32,uint256)
        let deposit_received_sig = keccak256(b"DepositReceived(address,bytes32,uint256)");
        info!(
            "🔍 Listening for DepositReceived events on contract: {}",
            contract_address
        );

        let mut iteration_count = 0;
        loop {
            iteration_count += 1;

            // Get current block number
            let current_block = match provider.get_block_number().await {
                Ok(block) => block,
                Err(e) => {
                    error!(
                        "❌ Failed to get current block number for chain {}: {}",
                        chain_config.name, e
                    );
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            // Log periodic status updates
            if iteration_count % 12 == 1 {
                // Every ~60 seconds (5s * 12)
                info!(
                    "💓 Chain {} heartbeat - Current block: {}, Last scanned: {}, Blocks behind: {}",
                    chain_config.name,
                    current_block,
                    last_block,
                    current_block.saturating_sub(last_block)
                );
            }

            // Catch up with historical blocks if needed
            if last_block < current_block {
                let blocks_behind = current_block - last_block;
                info!(
                    "⏭️ Chain {} catching up: {} blocks behind (from {} to {})",
                    chain_config.name, blocks_behind, last_block, current_block
                );

                // Process blocks in batches to avoid overwhelming the RPC
                const BATCH_SIZE: u64 = 100;
                let mut batch_start = last_block + 1;
                let mut total_processed = 0;

                while batch_start <= current_block {
                    let batch_end = std::cmp::min(batch_start + BATCH_SIZE - 1, current_block);

                    info!(
                        "📦 Processing batch {}-{} for chain {} ({}/{})",
                        batch_start,
                        batch_end,
                        chain_config.name,
                        total_processed + (batch_end - batch_start + 1),
                        blocks_behind
                    );

                    match Self::process_block_range(
                        &provider,
                        &chain_config,
                        &database,
                        contract_address,
                        deposit_received_sig,
                        batch_start,
                        batch_end,
                    )
                    .await
                    {
                        Ok(events_found) => {
                            last_block = batch_end;
                            total_processed += batch_end - batch_start + 1;

                            database
                                .update_chain_state(
                                    chain_config.chain_id,
                                    &chain_config.payment_vault_contract,
                                    last_block as i64,
                                )
                                .await?;

                            if events_found > 0 {
                                info!(
                                    "✨ Chain {} - Batch complete: {} events found in blocks {}-{}",
                                    chain_config.name, events_found, batch_start, batch_end
                                );
                            }
                        }
                        Err(e) => {
                            error!(
                                "❌ Error processing block range {}-{} for chain {}: {}",
                                batch_start, batch_end, chain_config.name, e
                            );
                            sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    }

                    batch_start = batch_end + 1;
                }

                info!(
                    "✅ Chain {} fully caught up! Processed {} blocks",
                    chain_config.name, total_processed
                );
            }

            // Wait before next iteration
            sleep(Duration::from_secs(5)).await;
        }
    }

    async fn process_block_range(
        provider: &dyn Provider<Ethereum>,
        chain_config: &ChainConfig,
        database: &Arc<Database>,
        contract_address: Address,
        event_signature: FixedBytes<32>,
        from_block: u64,
        to_block: u64,
    ) -> Result<usize> {
        let filter = Filter::new()
            .address(contract_address)
            .event_signature(event_signature)
            .from_block(from_block)
            .to_block(to_block);

        let logs = provider.get_logs(&filter).await?;

        if logs.is_empty() {
            debug!(
                "No events found in blocks {}-{} for chain {}",
                from_block, to_block, chain_config.name
            );
        } else {
            info!(
                "🎯 Found {} DepositReceived events in blocks {}-{} for chain {}",
                logs.len(),
                from_block,
                to_block,
                chain_config.name
            );
        }

        for (i, log) in logs.iter().enumerate() {
            info!(
                "📝 Processing event {}/{} in block {} for chain {}",
                i + 1,
                logs.len(),
                log.block_number.unwrap_or(0),
                chain_config.name
            );

            if let Err(e) = Self::process_deposit_event(chain_config, database, log).await {
                error!(
                    "❌ Error processing deposit event {}/{}: {}",
                    i + 1,
                    logs.len(),
                    e
                );
            }
        }

        Ok(logs.len())
    }

    async fn process_deposit_event(
        chain_config: &ChainConfig,
        database: &Arc<Database>,
        log: &Log,
    ) -> Result<()> {
        info!(
            "🔍 Parsing deposit event from tx: {} (block: {}, log_index: {})",
            log.transaction_hash
                .map(|h| h.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            log.block_number.unwrap_or(0),
            log.log_index.unwrap_or(0)
        );

        // Parse the event data
        let parsed_event = Self::parse_deposit_event(log)?;

        info!(
            "💰 Deposit event parsed - Order: {}, Sender: {}, Amount: {}",
            hex::encode(parsed_event.order_id),
            parsed_event.sender,
            parsed_event.amount
        );

        // Convert to database format
        let deposit_event = DepositEvent {
            id: None,
            chain_id: chain_config.chain_id as i64,
            contract_address: chain_config.payment_vault_contract.clone(),
            order_id: hex::encode(parsed_event.order_id),
            sender: parsed_event.sender.to_string(),
            amount: parsed_event.amount.to_string(),
            transaction_hash: parsed_event.transaction_hash,
            block_number: parsed_event.block_number as i64,
            log_index: parsed_event.log_index as i64,
            created_at: None,
            processed: false,
        };

        // Insert into database
        database.insert_deposit_event(&deposit_event).await?;

        // Update order with deposit details including status, amount, and chain_id
        database
            .update_order_with_deposit_details(
                &deposit_event.order_id,
                &deposit_event.transaction_hash,
                &deposit_event.amount,
                deposit_event.chain_id,
            )
            .await?;

        info!(
            "✅ Successfully processed deposit event for order {} on chain {} - Updated with amount {}, chain_id {}",
            deposit_event.order_id, chain_config.name, deposit_event.amount, chain_config.chain_id
        );

        Ok(())
    }

    fn parse_deposit_event(log: &Log) -> Result<ParsedDepositEvent> {
        debug!(
            "Parsing deposit event from log with {} topics",
            log.topics().len()
        );

        // Extract data from log - DepositReceived(address indexed payer, bytes32 indexed orderId, uint256 amount)
        // Topic[0] = event signature
        // Topic[1] = payer (address, indexed)
        // Topic[2] = orderId (bytes32, indexed)
        // Data = amount (uint256, not indexed)

        let payer = log
            .topics()
            .get(1)
            .ok_or_else(|| eyre::eyre!("Missing payer topic"))?;

        let order_id = log
            .topics()
            .get(2)
            .ok_or_else(|| eyre::eyre!("Missing order_id topic"))?;

        // Amount is in the data field (first 32 bytes)
        if log.data().data.len() < 32 {
            return Err(eyre::eyre!(
                "Invalid log data length: {}",
                log.data().data.len()
            ));
        }
        let amount = U256::from_be_slice(&log.data().data[..32]);

        let parsed_event = ParsedDepositEvent {
            order_id: order_id.0,
            sender: Address::from_slice(&payer.0[12..32]),
            amount,
            transaction_hash: log
                .transaction_hash
                .map(|h| h.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            block_number: log.block_number.unwrap_or(0),
            log_index: log.log_index.unwrap_or(0),
        };

        debug!(
            "Successfully parsed deposit event: order_id={}, sender={}, amount={}",
            hex::encode(parsed_event.order_id),
            parsed_event.sender,
            parsed_event.amount
        );

        Ok(parsed_event)
    }
}
