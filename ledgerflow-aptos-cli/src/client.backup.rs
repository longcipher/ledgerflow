use eyre::{eyre, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{debug, info};

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct VaultClient {
    http_client: Client,
    config: Config,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountResource {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub data: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionPayload {
    #[serde(rename = "type")]
    pub payload_type: String,
    pub function: String,
    pub arguments: Vec<String>,
    pub type_arguments: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitTransactionRequest {
    pub sender: String,
    pub sequence_number: String,
    pub max_gas_amount: String,
    pub gas_unit_price: String,
    pub expiration_timestamp_secs: String,
    pub payload: TransactionPayload,
    pub signature: TransactionSignature,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionSignature {
    #[serde(rename = "type")]
    pub signature_type: String,
    pub signature: String,
    pub public_key: String,
}

impl VaultClient {
    /// Create a new vault client from configuration
    pub fn new(config: Config) -> Result<Self> {
        // Parse private key
        let private_key_hex = config.account.private_key.trim_start_matches("0x");
        let private_key_bytes = hex::decode(private_key_hex)
            .context("Failed to decode private key hex")?;
        
        let private_key = Ed25519PrivateKey::try_from(private_key_bytes.as_slice())
            .context("Invalid private key format")?;

        // Create local account
        let account = LocalAccount::new(
            private_key.public_key().authentication_key().account_address(),
            private_key,
            0,
        );

        // Create REST client
        let client = Client::builder(AptosBaseUrl::Custom(config.network.node_url.clone()))
            .build();

        // Create transaction factory
        let chain_id = ChainId::new(config.network.chain_id);
        let transaction_factory = TransactionFactory::new(chain_id)
            .with_max_gas_amount(config.transaction.max_gas)
            .with_transaction_expiration_time(config.transaction.expiration_secs);

        let transaction_factory = if let Some(gas_price) = config.transaction.gas_unit_price {
            transaction_factory.with_gas_unit_price(gas_price)
        } else {
            transaction_factory
        };

        Ok(Self {
            client,
            account,
            transaction_factory,
            config,
        })
    }

    /// Get account address
    pub fn account_address(&self) -> AccountAddress {
        self.account.address()
    }

    /// Get account balance (USDC)
    pub async fn get_usdc_balance(&self) -> Result<u64> {
        // For this example, we'll use APT balance since USDC integration requires
        // more complex setup. In production, you'd query the USDC coin balance.
        let balance = self.client
            .get_account_balance(self.account.address())
            .await
            .context("Failed to get account balance")?;
        Ok(balance.into_inner().coin.value)
    }

    /// Get vault balance
    pub async fn get_vault_balance(&self) -> Result<u64> {
        let vault_address = AccountAddress::from_str(&self.config.vault.contract_address)
            .context("Invalid vault contract address")?;

        // Call the get_balance view function
        let response = self.client
            .view(
                &vault_address,
                &self.config.vault.module_name,
                "get_balance",
                vec![],
                vec![format!("0x{}", vault_address.to_hex())],
            )
            .await
            .context("Failed to call get_balance view function")?;

        // Parse the response as u64
        if let Some(balance_value) = response.into_inner().first() {
            let balance: u64 = serde_json::from_value(balance_value.clone())
                .context("Failed to deserialize balance")?;
            Ok(balance)
        } else {
            eyre::bail!("No balance returned from view function");
        }
    }

    /// Deposit USDC to the vault
    pub async fn deposit(&mut self, order_id: &str, amount: u64) -> Result<String> {
        info!(
            account = %self.account.address(),
            order_id = %order_id,
            amount = %amount,
            "Initiating vault deposit"
        );

        // Update account sequence number
        self.update_sequence_number().await?;

        // Parse vault address
        let vault_address = AccountAddress::from_str(&self.config.vault.contract_address)
            .context("Invalid vault contract address")?;

        // Convert order_id to bytes
        let order_id_bytes = order_id.as_bytes().to_vec();

        // Create entry function for deposit
        let module_id = ModuleId::new(
            vault_address,
            Identifier::new(&self.config.vault.module_name)
                .context("Invalid module name")?,
        );

        let entry_function = EntryFunction::new(
            module_id,
            Identifier::new("deposit").context("Invalid function name")?,
            vec![], // No type arguments
            vec![
                bcs::to_bytes(&vault_address)
                    .context("Failed to serialize vault address")?,
                bcs::to_bytes(&order_id_bytes)
                    .context("Failed to serialize order ID")?,
                bcs::to_bytes(&amount)
                    .context("Failed to serialize amount")?,
            ],
        );

        // Build and sign transaction
        let transaction_builder = self.transaction_factory
            .entry_function(entry_function);

        let signed_transaction = self.account
            .sign_with_transaction_builder(transaction_builder);

        debug!(
            transaction_hash = %signed_transaction.committed_hash(),
            "Transaction signed, submitting to network"
        );

        // Submit transaction
        if self.config.transaction.wait_for_transaction {
            let response = self.client
                .submit_and_wait_bcs(&signed_transaction)
                .await
                .context("Failed to submit and wait for transaction")?;
            
            let tx_hash = response.into_inner().info.hash.to_string();
            info!(
                transaction_hash = %tx_hash,
                "Deposit transaction completed successfully"
            );
            Ok(tx_hash)
        } else {
            let response = self.client
                .submit_bcs(&signed_transaction)
                .await
                .context("Failed to submit transaction")?;
            
            let tx_hash = response.into_inner().hash.to_string();
            info!(
                transaction_hash = %tx_hash,
                "Deposit transaction submitted"
            );
            Ok(tx_hash)
        }
    }

    /// Withdraw USDC from the vault
    pub async fn withdraw(&mut self, recipient: &str, amount: u64) -> Result<String> {
        info!(
            account = %self.account.address(),
            recipient = %recipient,
            amount = %amount,
            "Initiating vault withdrawal"
        );

        // Update account sequence number
        self.update_sequence_number().await?;

        // Parse addresses
        let vault_address = AccountAddress::from_str(&self.config.vault.contract_address)
            .context("Invalid vault contract address")?;
        let recipient_address = AccountAddress::from_str(recipient)
            .context("Invalid recipient address")?;

        // Create entry function for withdraw
        let module_id = ModuleId::new(
            vault_address,
            Identifier::new(&self.config.vault.module_name)
                .context("Invalid module name")?,
        );

        let entry_function = EntryFunction::new(
            module_id,
            Identifier::new("withdraw").context("Invalid function name")?,
            vec![], // No type arguments
            vec![
                bcs::to_bytes(&vault_address)
                    .context("Failed to serialize vault address")?,
                bcs::to_bytes(&recipient_address)
                    .context("Failed to serialize recipient address")?,
                bcs::to_bytes(&amount)
                    .context("Failed to serialize amount")?,
            ],
        );

        // Build and sign transaction
        let transaction_builder = self.transaction_factory
            .entry_function(entry_function);

        let signed_transaction = self.account
            .sign_with_transaction_builder(transaction_builder);

        debug!(
            transaction_hash = %signed_transaction.committed_hash(),
            "Transaction signed, submitting to network"
        );

        // Submit transaction
        if self.config.transaction.wait_for_transaction {
            let response = self.client
                .submit_and_wait_bcs(&signed_transaction)
                .await
                .context("Failed to submit and wait for transaction")?;
            
            let tx_hash = response.into_inner().info.hash.to_string();
            info!(
                transaction_hash = %tx_hash,
                "Withdrawal transaction completed successfully"
            );
            Ok(tx_hash)
        } else {
            let response = self.client
                .submit_bcs(&signed_transaction)
                .await
                .context("Failed to submit transaction")?;
            
            let tx_hash = response.into_inner().hash.to_string();
            info!(
                transaction_hash = %tx_hash,
                "Withdrawal transaction submitted"
            );
            Ok(tx_hash)
        }
    }

    /// Withdraw all funds from the vault
    pub async fn withdraw_all(&mut self, recipient: &str) -> Result<String> {
        info!(
            account = %self.account.address(),
            recipient = %recipient,
            "Initiating vault withdrawal (all funds)"
        );

        // Update account sequence number
        self.update_sequence_number().await?;

        // Parse addresses
        let vault_address = AccountAddress::from_str(&self.config.vault.contract_address)
            .context("Invalid vault contract address")?;
        let recipient_address = AccountAddress::from_str(recipient)
            .context("Invalid recipient address")?;

        // Create entry function for withdraw_all
        let module_id = ModuleId::new(
            vault_address,
            Identifier::new(&self.config.vault.module_name)
                .context("Invalid module name")?,
        );

        let entry_function = EntryFunction::new(
            module_id,
            Identifier::new("withdraw_all").context("Invalid function name")?,
            vec![], // No type arguments
            vec![
                bcs::to_bytes(&vault_address)
                    .context("Failed to serialize vault address")?,
                bcs::to_bytes(&recipient_address)
                    .context("Failed to serialize recipient address")?,
            ],
        );

        // Build and sign transaction
        let transaction_builder = self.transaction_factory
            .entry_function(entry_function);

        let signed_transaction = self.account
            .sign_with_transaction_builder(transaction_builder);

        debug!(
            transaction_hash = %signed_transaction.committed_hash(),
            "Transaction signed, submitting to network"
        );

        // Submit transaction
        if self.config.transaction.wait_for_transaction {
            let response = self.client
                .submit_and_wait_bcs(&signed_transaction)
                .await
                .context("Failed to submit and wait for transaction")?;
            
            let tx_hash = response.into_inner().info.hash.to_string();
            info!(
                transaction_hash = %tx_hash,
                "Withdrawal-all transaction completed successfully"
            );
            Ok(tx_hash)
        } else {
            let response = self.client
                .submit_bcs(&signed_transaction)
                .await
                .context("Failed to submit transaction")?;
            
            let tx_hash = response.into_inner().hash.to_string();
            info!(
                transaction_hash = %tx_hash,
                "Withdrawal-all transaction submitted"
            );
            Ok(tx_hash)
        }
    }

    /// Get vault information
    pub async fn get_vault_info(&self) -> Result<VaultInfo> {
        let vault_address = AccountAddress::from_str(&self.config.vault.contract_address)
            .context("Invalid vault contract address")?;

        // Get balance
        let balance = self.get_vault_balance().await?;

        // Get owner
        let owner_response = self.client
            .view(
                &vault_address,
                &self.config.vault.module_name,
                "get_owner",
                vec![],
                vec![format!("0x{}", vault_address.to_hex())],
            )
            .await
            .context("Failed to call get_owner view function")?;

        let owner = if let Some(owner_value) = owner_response.into_inner().first() {
            let owner_addr_str: String = serde_json::from_value(owner_value.clone())
                .context("Failed to deserialize owner address")?;
            AccountAddress::from_str(&owner_addr_str)
                .context("Invalid owner address format")?
        } else {
            eyre::bail!("No owner returned from view function");
        };

        // Get deposit count
        let deposit_count_response = self.client
            .view(
                &vault_address,
                &self.config.vault.module_name,
                "get_deposit_count",
                vec![],
                vec![format!("0x{}", vault_address.to_hex())],
            )
            .await
            .context("Failed to call get_deposit_count view function")?;

        let deposit_count = if let Some(count_value) = deposit_count_response.into_inner().first() {
            let count: u64 = serde_json::from_value(count_value.clone())
                .context("Failed to deserialize deposit count")?;
            count
        } else {
            eyre::bail!("No deposit count returned from view function");
        };

        Ok(VaultInfo {
            address: vault_address,
            balance,
            owner,
            deposit_count,
        })
    }

    /// Update account sequence number from chain
    async fn update_sequence_number(&mut self) -> Result<()> {
        let account_info = self.client
            .get_account(self.account.address())
            .await
            .context("Failed to get account information")?;
        
        self.account.set_sequence_number(account_info.into_inner().sequence_number);
        debug!(
            account = %self.account.address(),
            sequence_number = %self.account.sequence_number(),
            "Updated account sequence number"
        );
        Ok(())
    }
}

/// Vault information structure
#[derive(Debug)]
pub struct VaultInfo {
    pub address: AccountAddress,
    pub balance: u64,
    pub owner: AccountAddress,
    pub deposit_count: u64,
}
