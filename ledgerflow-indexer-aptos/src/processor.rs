use eyre::Result;

use crate::config::Config;

pub async fn run_indexer(_config: Config) -> Result<()> {
    Err(eyre::eyre!(
        "ledgerflow-indexer-aptos is not production-ready: event extraction and DB persistence are not implemented"
    ))
}
