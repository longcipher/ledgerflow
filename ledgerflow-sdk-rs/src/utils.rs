// LedgerFlow SDK utility functions

use sha3::{Digest, Keccak256};

/// Generate a unique order ID using keccak256 hash.
///
/// Replicates `abi.encodePacked(broker_id, account_id, order_id_num)` from the
/// EVM smart-contract side: the broker string bytes are concatenated with the
/// big-endian `i64` representations of `account_id` and `order_id_num`, then
/// hashed with Keccak-256. The result is returned as a lowercase hex string
/// (64 characters / 32 bytes).
///
/// This is identical to `ledgerflow-balancer/src/utils.rs::generate_order_id`.
pub fn generate_order_id(broker_id: &str, account_id: i64, order_id_num: i64) -> String {
    let mut hasher = Keccak256::new();

    // Encode the parameters (matches Solidity's abi.encodePacked)
    hasher.update(broker_id.as_bytes());
    hasher.update(account_id.to_be_bytes());
    hasher.update(order_id_num.to_be_bytes());

    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standard case — must match the balancer's output for the same inputs.
    /// Hash pinned so any algorithm change is caught immediately.
    #[test]
    fn test_generate_order_id_standard() {
        let hash = generate_order_id("ledgerflow", 1, 1);
        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "f6a9b4070b02024a5d1e8e278618409cf219dfb7013415b1786896b24a2799e5",
        );
    }

    /// Different broker, larger numbers — pinned hash.
    #[test]
    fn test_generate_order_id_different_broker() {
        let hash = generate_order_id("test-broker", 12345, 67890);
        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "d680be90b2657b8ab2970f58808d80813b28a9c0aaae0256e7795d85847ee6d5",
        );
    }

    /// Edge case — zero account_id and order_id_num.
    #[test]
    fn test_generate_order_id_zeros() {
        let hash = generate_order_id("ledgerflow", 0, 0);
        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "11421fae289117a703aad9563804a537596d7aee730ef9aedfbb36c06e0a0a9c",
        );
    }

    /// Edge case — empty broker_id.
    #[test]
    fn test_generate_order_id_empty_broker() {
        let hash = generate_order_id("", 1, 1);
        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "5dd50243f81eaa0bd39ace71862b46f2054c3ea1c2b69a79093b5795061e3851",
        );
    }

    /// Edge case — i64::MAX for both numeric fields.
    #[test]
    fn test_generate_order_id_max_values() {
        let hash = generate_order_id("ledgerflow", i64::MAX, i64::MAX);
        assert_eq!(hash.len(), 64);
        assert_eq!(
            hash,
            "c9cc66b2214b27ec39a31485d20023c46852da0d521f0f7aa2457ccdc0157bc3",
        );
    }

    /// Changing only account_id produces a different hash.
    #[test]
    fn test_generate_order_id_different_account() {
        let a = generate_order_id("ledgerflow", 1, 1);
        let b = generate_order_id("ledgerflow", 2, 1);
        assert_ne!(a, b);
    }

    /// Changing only order_id_num produces a different hash.
    #[test]
    fn test_generate_order_id_different_order_num() {
        let a = generate_order_id("ledgerflow", 1, 1);
        let b = generate_order_id("ledgerflow", 1, 2);
        assert_ne!(a, b);
    }

    /// Negative values are valid i64 — verify the function handles them
    /// deterministically (big-endian two's complement).
    #[test]
    fn test_generate_order_id_negative_values() {
        let hash = generate_order_id("ledgerflow", -1, -1);
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, generate_order_id("ledgerflow", 1, 1));
    }
}
