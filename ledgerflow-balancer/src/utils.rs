use sha3::{Digest, Keccak256};

/// Generate a unique order ID using keccak256 hash
/// order_id = keccak256(abi.encodePacked(broker_id, account_id, order_id_num))
pub fn generate_order_id(broker_id: &str, account_id: &str, order_id_num: u64) -> String {
    let mut hasher = Keccak256::new();

    // Encode the parameters (similar to abi.encodePacked)
    hasher.update(broker_id.as_bytes());
    hasher.update(account_id.as_bytes());
    hasher.update(order_id_num.to_be_bytes());

    let result = hasher.finalize();
    hex::encode(result)
}

/// Get the next order ID number for an account
/// This is a simple implementation that could be enhanced with better sequencing
pub fn get_next_order_id_num(account_id: &str) -> u64 {
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };

    let mut hasher = DefaultHasher::new();
    account_id.hash(&mut hasher);
    let base = hasher.finish();

    // Add current timestamp to ensure uniqueness
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    base.wrapping_add(timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_order_id() {
        let broker_id = "test_broker";
        let account_id = "test_account";
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
    fn test_get_next_order_id_num() {
        let account_id = "test_account";

        let num1 = get_next_order_id_num(account_id);
        let num2 = get_next_order_id_num(account_id);

        // Should be different due to timestamp
        assert_ne!(num1, num2);
    }
}
