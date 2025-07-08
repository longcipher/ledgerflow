use std::str::FromStr;

use alloy::{
    network::EthereumWallet,
    primitives::{Address, FixedBytes, U256},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
};
use eyre::{Result, eyre};

/// Parse hex string to address
pub fn parse_address(addr_str: &str) -> Result<Address> {
    let addr_str = if let Some(stripped) = addr_str.strip_prefix("0x") {
        stripped
    } else {
        addr_str
    };

    Address::from_str(addr_str).map_err(|e| eyre!("Invalid address format: {}", e))
}

/// Parse hex string to FixedBytes<32> (for order ID)
pub fn parse_order_id(order_id_str: &str) -> Result<FixedBytes<32>> {
    let order_id_str = if let Some(stripped) = order_id_str.strip_prefix("0x") {
        stripped
    } else {
        order_id_str
    };

    if order_id_str.len() != 64 {
        return Err(eyre!("Order ID must be 32 bytes (64 hex characters)"));
    }

    FixedBytes::from_str(order_id_str).map_err(|e| eyre!("Invalid order ID format: {}", e))
}

/// Parse private key and create signer
pub fn parse_private_key(private_key_str: &str) -> Result<PrivateKeySigner> {
    let private_key_str = if let Some(stripped) = private_key_str.strip_prefix("0x") {
        stripped
    } else {
        private_key_str
    };

    PrivateKeySigner::from_str(private_key_str)
        .map_err(|e| eyre!("Invalid private key format: {}", e))
}

/// Create provider with wallet
pub async fn create_provider_with_wallet(
    rpc_url: &str,
    private_key: &str,
) -> Result<impl alloy::providers::Provider + Clone> {
    let signer = parse_private_key(private_key)?;
    let wallet = EthereumWallet::from(signer);

    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect(rpc_url)
        .await?;

    Ok(provider)
}

/// Format amount for display (assuming USDC has 6 decimals)
pub fn format_usdc_amount(amount: U256) -> String {
    let amount_u64 = amount.to::<u64>();
    let whole = amount_u64 / 1_000_000;
    let decimal = amount_u64 % 1_000_000;
    format!("{whole}.{decimal:06}")
}

/// Parse USDC amount from string (e.g. "1.5" -> 1500000)
#[allow(dead_code)]
pub fn parse_usdc_amount(amount_str: &str) -> Result<U256> {
    let amount: f64 = amount_str.parse()?;
    let amount_scaled = (amount * 1_000_000.0) as u64;
    Ok(U256::from(amount_scaled))
}
