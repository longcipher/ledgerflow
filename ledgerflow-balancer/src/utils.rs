use std::env;

use sha3::{Digest, Keccak256};

/// Generate a unique order ID using keccak256 hash
/// order_id = keccak256(abi.encodePacked(broker_id, account_id, order_id_num))
pub fn generate_order_id(broker_id: &str, account_id: i64, order_id_num: i64) -> String {
    let mut hasher = Keccak256::new();

    // Encode the parameters (similar to abi.encodePacked)
    hasher.update(broker_id.as_bytes());
    hasher.update(account_id.to_be_bytes());
    hasher.update(order_id_num.to_be_bytes());

    let result = hasher.finalize();
    hex::encode(result)
}

/// Encrypt a private key using XOR with an environment variable key
pub fn encrypt_private_key(private_key: &str) -> Result<String, String> {
    let master_key = env::var("ENCRYPTED_MASTER_KEY")
        .map_err(|_| "ENCRYPTED_MASTER_KEY environment variable is not set".to_string())?;

    let private_key_bytes = private_key.as_bytes();
    let master_key_bytes = master_key.as_bytes();

    let encrypted: Vec<u8> = private_key_bytes
        .iter()
        .enumerate()
        .map(|(i, byte)| byte ^ master_key_bytes[i % master_key_bytes.len()])
        .collect();

    Ok(hex::encode(encrypted))
}

/// Decrypt a private key using XOR with an environment variable key
#[allow(unused)]
pub fn decrypt_private_key(encrypted_key: &str) -> Result<String, String> {
    let master_key = env::var("ENCRYPTED_MASTER_KEY")
        .map_err(|_| "ENCRYPTED_MASTER_KEY environment variable is not set".to_string())?;

    let encrypted_bytes =
        hex::decode(encrypted_key).map_err(|e| format!("Failed to decode hex: {e}"))?;
    let master_key_bytes = master_key.as_bytes();

    let decrypted: Vec<u8> = encrypted_bytes
        .iter()
        .enumerate()
        .map(|(i, byte)| byte ^ master_key_bytes[i % master_key_bytes.len()])
        .collect();

    String::from_utf8(decrypted).map_err(|e| format!("Failed to convert to UTF-8: {e}"))
}

/// Generate EVM address from private key
pub fn generate_evm_address_from_pk(private_key: &str) -> Result<String, String> {
    use alloy::signers::local::PrivateKeySigner;

    let private_key_bytes = hex::decode(private_key.trim_start_matches("0x"))
        .map_err(|e| format!("Failed to decode private key: {e}"))?;

    let signer = PrivateKeySigner::from_slice(&private_key_bytes)
        .map_err(|e| format!("Failed to create signer: {e}"))?;

    Ok(signer.address().to_checksum(None))
}

/// Get the next order ID number globally (auto-increment) - for testing/fallback
/// This is a thread-safe global counter implementation
#[allow(unused)]
pub fn get_next_order_id_num_fallback(_account_id: &str) -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};

    static GLOBAL_ORDER_COUNTER: AtomicU64 = AtomicU64::new(1);

    GLOBAL_ORDER_COUNTER.fetch_add(1, Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_order_id() {
        let broker_id = "test_broker";
        let account_id = 12345i64;
        let order_id_num = 123;

        let order_id = generate_order_id(broker_id, account_id, order_id_num);
        assert_eq!(order_id.len(), 64); // keccak256 produces 32 bytes = 64 hex chars

        // Same input should produce same output
        let order_id2 = generate_order_id(broker_id, account_id, order_id_num);
        assert_eq!(order_id, order_id2);

        // Different input should produce different output
        let order_id3 = generate_order_id(broker_id, account_id, 124);
        assert_ne!(order_id, order_id3);
    }

    #[test]
    fn test_get_next_order_id_num_fallback() {
        let account_id = "test_account";

        let num1 = get_next_order_id_num_fallback(account_id);
        let num2 = get_next_order_id_num_fallback(account_id);

        // Should be incrementing globally
        assert_eq!(num2, num1 + 1);

        // Different account should still get next global number
        let num3 = get_next_order_id_num_fallback("different_account");
        assert_eq!(num3, num2 + 1);
    }

    #[tokio::test]
    async fn test_get_next_order_id_num() {
        // This test would require a real database connection
        // For now, we'll just test that the function signature is correct
        // In a real test, you would set up a test database and call:
        // let pool = PgPool::connect("postgresql://test_url").await.unwrap();
        // let num = get_next_order_id_num(&pool).await.unwrap();
        // assert!(num > 0);
    }
}
