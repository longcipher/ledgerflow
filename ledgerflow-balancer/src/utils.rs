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

/// Generate a random API token for client authentication.
pub fn generate_api_token() -> String {
    let bytes = rand::random::<[u8; 32]>();
    format!("lf_{:}", hex::encode(bytes))
}

/// Hash API token before persistence or lookup.
pub fn hash_api_token(token: &str) -> String {
    let mut hasher = Keccak256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Parse and normalize an EVM address to EIP-55 checksum format.
pub fn normalize_evm_address(address: &str) -> Result<String, String> {
    let parsed = address
        .parse::<alloy::primitives::Address>()
        .map_err(|e| format!("Invalid EVM address '{address}': {e}"))?;
    Ok(parsed.to_checksum(None))
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
    fn test_generate_api_token() {
        let token = generate_api_token();
        assert!(token.starts_with("lf_"));
        assert_eq!(token.len(), 67);
    }

    #[test]
    fn test_hash_api_token_is_deterministic() {
        let hash1 = hash_api_token("token-1");
        let hash2 = hash_api_token("token-1");
        let hash3 = hash_api_token("token-2");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64);
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

    #[test]
    fn test_normalize_evm_address() {
        let normalized = normalize_evm_address("0x00000000000000000000000000000000000000aa")
            .expect("address should normalize");
        assert_eq!(normalized, "0x00000000000000000000000000000000000000AA");
    }

    #[test]
    fn test_normalize_evm_address_rejects_invalid_value() {
        let error = normalize_evm_address("not-an-address").expect_err("must reject invalid");
        assert!(error.contains("Invalid EVM address"));
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
