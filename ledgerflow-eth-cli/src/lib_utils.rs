use std::str::FromStr;

use alloy::{
    network::EthereumWallet,
    primitives::{Address, FixedBytes, U256},
    providers::ProviderBuilder,
    signers::{Signer, local::PrivateKeySigner},
    sol,
    sol_types::{SolStruct, eip712_domain},
};
use eyre::{Result, eyre};

// Define the Permit struct for EIP-712 signing
sol! {
    #[allow(missing_docs)]
    struct Permit {
        address owner;
        address spender;
        uint256 value;
        uint256 nonce;
        uint256 deadline;
    }
}

/// Domain configuration for EIP-712 signing
pub struct DomainConfig {
    pub chain_id: u64,
    pub name: String,
    pub version: String,
    pub verifying_contract: Address,
}

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

/// Create EIP-712 signature for USDC permit
pub async fn create_permit_signature(
    signer: &PrivateKeySigner,
    owner: Address,
    spender: Address,
    value: U256,
    nonce: U256,
    deadline: U256,
    domain_config: DomainConfig,
) -> Result<(u8, FixedBytes<32>, FixedBytes<32>)> {
    println!("EIP-712 Domain used:");
    println!("  name: {}", domain_config.name);
    println!("  version: {}", domain_config.version);
    println!("  chainId: {}", domain_config.chain_id);
    println!("  verifyingContract: {}", domain_config.verifying_contract);

    // Create the EIP-712 domain for USDC using actual contract values
    let domain = eip712_domain! {
        name: domain_config.name,
        version: domain_config.version,
        chain_id: domain_config.chain_id,
        verifying_contract: domain_config.verifying_contract,
    };

    // Create the permit struct
    let permit = Permit {
        owner,
        spender,
        value,
        nonce,
        deadline,
    };

    // Get the EIP-712 signing hash
    let hash = permit.eip712_signing_hash(&domain);

    // Sign the hash
    let signature = signer.sign_hash(&hash).await?;

    // Extract v, r, s from signature
    let v = if signature.v() { 28u8 } else { 27u8 };
    let r = FixedBytes::from(signature.r().to_be_bytes::<32>());
    let s = FixedBytes::from(signature.s().to_be_bytes::<32>());

    Ok((v, r, s))
}
