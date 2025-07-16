use aptos_indexer_processor_sdk::postgres::basic_processor::process;
use diesel_migrations::{EmbeddedMigrations, embed_migrations};
use eyre::Result;
use tracing::info;

use crate::config::Config;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

pub async fn run_indexer(_config: Config) -> Result<()> {
    let processor_name = "ledgerflow-aptos-indexer".to_string();
    info!("Starting Aptos indexer processor: {}", processor_name);

    process(processor_name, MIGRATIONS, |_txns, _conn_pool| async move {
        // ...existing code...

        // TODO: Find correct event extraction path from txn, then restore event loop and DB insert logic.
        // for txn in txns {
        //     ...
        // }
        Ok(())
    })
    .await
    .map_err(|e| eyre::eyre!(e))?;
    Ok(())
}
