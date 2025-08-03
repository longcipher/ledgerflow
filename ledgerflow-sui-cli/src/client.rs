use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use sui_keys::keystore::InMemKeystore;
use sui_sdk::{
    SuiClient, SuiClientBuilder,
    types::base_types::{ObjectID, SuiAddress},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub hash: String,
    pub success: bool,
    pub gas_used: u64,
    pub timestamp: i64,
}

#[allow(dead_code)]
pub struct VaultClient {
    client: SuiClient,
    keystore: InMemKeystore,
    active_address: SuiAddress,
    package_id: ObjectID,
    vault_object_id: ObjectID,
    usdc_type: String,
    gas_budget: u64,
}

impl VaultClient {
    pub fn new(
        rpc_url: String,
        private_key: String,
        package_id: String,
        vault_object_id: String,
        usdc_type: String,
        gas_budget: u64,
    ) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new().context("Failed to create runtime")?;
        let client = rt
            .block_on(async { SuiClientBuilder::default().build(&rpc_url).await })
            .context("Failed to build Sui client")?;

        // Create a dummy keystore for now
        let keystore = InMemKeystore::default();

        // Parse the private key (simplified - in practice you'd properly handle key creation)
        let active_address = if private_key.starts_with("0x") && private_key.len() == 66 {
            // Try to parse as a 32-byte hex string and convert to SuiAddress
            let bytes = hex::decode(&private_key[2..]).context("Failed to decode private key")?;
            if bytes.len() == 32 {
                SuiAddress::from_bytes(&bytes[..20]).context("Failed to create SuiAddress")?
            } else {
                SuiAddress::random_for_testing_only()
            }
        } else {
            SuiAddress::random_for_testing_only()
        };

        let package_id = ObjectID::from_str(&package_id).context("Failed to parse package ID")?;
        let vault_object_id =
            ObjectID::from_str(&vault_object_id).context("Failed to parse vault object ID")?;

        Ok(VaultClient {
            client,
            keystore,
            active_address,
            package_id,
            vault_object_id,
            usdc_type,
            gas_budget,
        })
    }

    pub async fn deposit(&mut self, amount: u64, order_id: Vec<u8>) -> Result<TransactionResult> {
        tracing::info!(
            amount = amount,
            order_id = hex::encode(&order_id),
            "Building deposit transaction"
        );

        // This is a simplified implementation that returns a mock result
        // In a real implementation, you would:
        // 1. Build a programmable transaction block
        // 2. Call the vault's deposit function
        // 3. Sign and execute the transaction

        // For now, return a mock transaction result
        Ok(TransactionResult {
            hash: "0x1234567890abcdef".to_string(),
            success: true,
            gas_used: 1000,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Failed to get current time")
                .as_secs() as i64,
        })
    }

    pub async fn withdraw(&mut self, amount: u64, recipient: String) -> Result<TransactionResult> {
        tracing::info!(
            amount = amount,
            recipient = recipient,
            "Building withdraw transaction"
        );

        // Similar to deposit but calling withdraw function
        // This is a simplified implementation
        Ok(TransactionResult {
            hash: "0x1234567890abcdef".to_string(),
            success: true,
            gas_used: 1000,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Failed to get current time")
                .as_secs() as i64,
        })
    }

    pub async fn withdraw_all(&mut self, recipient: String) -> Result<TransactionResult> {
        tracing::info!(recipient = recipient, "Building withdraw all transaction");

        // Similar to withdraw but calling withdraw_all function
        Ok(TransactionResult {
            hash: "0x1234567890abcdef".to_string(),
            success: true,
            gas_used: 1000,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Failed to get current time")
                .as_secs() as i64,
        })
    }

    pub async fn get_balance(&self, account: &str) -> Result<u64> {
        let account_addr = SuiAddress::from_str(account)
            .map_err(|e| eyre::eyre!("Failed to parse account address: {}", e))?;

        let coins = self
            .client
            .coin_read_api()
            .get_coins(account_addr, Some(self.usdc_type.clone()), None, None)
            .await
            .context("Failed to get account balance")?;

        let total_balance: u64 = coins.data.iter().map(|coin| coin.balance).sum();
        Ok(total_balance)
    }

    /// Get account address
    pub fn account_address(&self) -> String {
        self.active_address.to_string()
    }

    /// Get vault information
    pub async fn get_vault_info(&self) -> Result<serde_json::Value> {
        // Get vault object information (simplified version)
        // In practice, you'd call get_object_with_options and parse the Move struct

        Ok(serde_json::json!({
            "vault_address": self.vault_object_id.to_string(),
            "total_deposit": 0, // Would need to read from vault struct fields
            "owner": self.active_address.to_string(),
            "created_at": "2024-01-01T00:00:00Z"
        }))
    }

    /// Get USDC balance (alias for get_balance)
    #[allow(dead_code)]
    pub async fn get_usdc_balance(&self, account: &str) -> Result<u64> {
        self.get_balance(account).await
    }

    /// Get SUI balance for gas
    pub async fn get_sui_balance(&self, account: &str) -> Result<u64> {
        let account_addr = SuiAddress::from_str(account)
            .map_err(|e| eyre::eyre!("Failed to parse account address: {}", e))?;

        let balance = self
            .client
            .coin_read_api()
            .get_balance(account_addr, None)
            .await
            .context("Failed to get SUI balance")?;

        Ok(balance.total_balance.try_into().unwrap_or(0))
    }
}
