use std::sync::Arc;

use alloy::{
    network::Ethereum,
    primitives::{Address, FixedBytes, U256, keccak256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{Filter, Log},
};
use eyre::Result;
use tokio::time::{Duration, sleep};
use tracing::{error, info};

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

        // Start indexing for each chain
        for chain in &self.config.chains {
            let chain_config = chain.clone();
            let database = Arc::clone(&self.database);

            let handle = tokio::spawn(async move {
                if let Err(e) = Self::index_chain(chain_config, database).await {
                    error!("Error indexing chain: {}", e);
                }
            });

            handles.push(handle);
        }

        // Wait for all indexing tasks to complete
        for handle in handles {
            handle.await?;
        }

        Ok(())
    }

    async fn index_chain(chain_config: ChainConfig, database: Arc<Database>) -> Result<()> {
        info!("Starting indexer for chain: {}", chain_config.name);

        // Create provider for this chain
        let provider = ProviderBuilder::new()
            .connect(&chain_config.rpc_http)
            .await?;

        // Get the last scanned block from database
        let mut last_block = match database
            .get_chain_state(&chain_config.name, &chain_config.payment_vault_contract)
            .await?
        {
            Some(state) => state.last_scanned_block as u64,
            None => chain_config.start_block,
        };

        info!(
            "Starting from block {} for chain {}",
            last_block, chain_config.name
        );

        // Contract address
        let contract_address: Address = chain_config.payment_vault_contract.parse()?;

        // DepositReceived event signature: DepositReceived(address,bytes32,uint256)
        let deposit_received_sig = keccak256(b"DepositReceived(address,bytes32,uint256)");

        loop {
            // Get current block number
            let current_block = match provider.get_block_number().await {
                Ok(block) => block,
                Err(e) => {
                    error!("Failed to get current block number: {}", e);
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            // Catch up with historical blocks if needed
            if last_block < current_block {
                info!(
                    "Catching up from block {} to {} for chain {}",
                    last_block, current_block, chain_config.name
                );

                // Process blocks in batches to avoid overwhelming the RPC
                const BATCH_SIZE: u64 = 100;
                let mut batch_start = last_block + 1;

                while batch_start <= current_block {
                    let batch_end = std::cmp::min(batch_start + BATCH_SIZE - 1, current_block);

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
                        Ok(_) => {
                            last_block = batch_end;
                            database
                                .update_chain_state(
                                    &chain_config.name,
                                    &chain_config.payment_vault_contract,
                                    last_block as i64,
                                )
                                .await?;
                        }
                        Err(e) => {
                            error!(
                                "Error processing block range {}-{}: {}",
                                batch_start, batch_end, e
                            );
                            sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    }

                    batch_start = batch_end + 1;
                }
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
    ) -> Result<()> {
        let filter = Filter::new()
            .address(contract_address)
            .event_signature(event_signature)
            .from_block(from_block)
            .to_block(to_block);

        let logs = provider.get_logs(&filter).await?;

        info!(
            "Found {} events in blocks {}-{} for chain {}",
            logs.len(),
            from_block,
            to_block,
            chain_config.name
        );

        for log in logs {
            if let Err(e) = Self::process_deposit_event(chain_config, database, &log).await {
                error!("Error processing deposit event: {}", e);
            }
        }

        Ok(())
    }

    async fn process_deposit_event(
        chain_config: &ChainConfig,
        database: &Arc<Database>,
        log: &Log,
    ) -> Result<()> {
        // Parse the event data
        let parsed_event = Self::parse_deposit_event(log)?;

        // Convert to database format
        let deposit_event = DepositEvent {
            id: None,
            chain_name: chain_config.name.clone(),
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

        info!(
            "Processed deposit event for order {} on chain {}",
            deposit_event.order_id, chain_config.name
        );

        Ok(())
    }

    fn parse_deposit_event(log: &Log) -> Result<ParsedDepositEvent> {
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
            return Err(eyre::eyre!("Invalid log data length"));
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

        Ok(parsed_event)
    }
}
