#![allow(unused)]

use alloy::{primitives::keccak256, signers::local::PrivateKeySigner};
use eyre::Result;
use rand::Rng;

use crate::{error::BotError, models::Wallet};

pub async fn generate_wallet() -> Result<Wallet> {
    // Generate a random private key
    let mut rng = rand::rng();
    let private_key_bytes: [u8; 32] = rng.random();

    // Create signer from private key
    let signer = PrivateKeySigner::from_bytes(&private_key_bytes.into())?;

    // Get the address
    let address = signer.address();

    // Convert private key to hex string
    let private_key_hex = hex::encode(private_key_bytes);

    Ok(Wallet {
        address: format!("0x{address:x}"),
        private_key: format!("0x{private_key_hex}"),
    })
}

pub fn validate_address(address: &str) -> Result<bool> {
    // Remove 0x prefix if present
    let address_str = address.strip_prefix("0x").unwrap_or(address);

    // Check if it's a valid hex string of correct length
    if address_str.len() != 40 {
        return Ok(false);
    }

    // Try to parse as hex
    hex::decode(address_str).map_err(|_| BotError::Wallet("Invalid address format".to_string()))?;

    Ok(true)
}

pub fn format_address(address: &str) -> String {
    if !address.starts_with("0x") {
        format!("0x{address}")
    } else {
        address.to_string()
    }
}

pub fn generate_order_id(broker_id: &str, account_id: &str, order_num: u64) -> String {
    // Create the input data for keccak256
    let input = format!("{broker_id}{account_id}{order_num}");
    let input_bytes = input.as_bytes();

    // Calculate keccak256 hash
    let hash = keccak256(input_bytes);

    // Convert to hex string
    format!("0x{}", hex::encode(hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_wallet() {
        let wallet = generate_wallet().await.unwrap();
        assert!(wallet.address.starts_with("0x"));
        assert!(wallet.private_key.starts_with("0x"));
        assert_eq!(wallet.address.len(), 42); // 0x + 40 hex chars
        assert_eq!(wallet.private_key.len(), 66); // 0x + 64 hex chars
    }

    #[test]
    fn test_validate_address() {
        assert!(validate_address("0x742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6").unwrap());
        assert!(validate_address("742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6").unwrap());
        assert!(!validate_address("0x742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD").unwrap());
        assert!(!validate_address("invalid").unwrap());
    }

    #[test]
    fn test_format_address() {
        assert_eq!(
            format_address("742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6"),
            "0x742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6"
        );
        assert_eq!(
            format_address("0x742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6"),
            "0x742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6"
        );
    }

    #[test]
    fn test_generate_order_id() {
        let order_id = generate_order_id("broker1", "account1", 1);
        assert!(order_id.starts_with("0x"));
        assert_eq!(order_id.len(), 66); // 0x + 64 hex chars
    }
}
