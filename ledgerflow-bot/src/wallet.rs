#![allow(unused)]
use std::env;

use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit, consts::U12},
};
use alloy::{
    primitives::{U256, keccak256},
    providers::Provider,
    signers::local::PrivateKeySigner,
    sol_types::SolEvent,
};
use eyre::{Result, eyre};
use rand::RngExt;

use crate::{
    contracts::{PaymentVault, USDC},
    error::BotError,
    lib_utils::{
        create_provider_with_wallet, format_usdc_amount, parse_address, parse_order_id,
        parse_private_key,
    },
    models::Wallet,
};

const MASTER_KEY_ENV_PRIMARY: &str = "WALLET_MASTER_KEY";
const MASTER_KEY_ENV_LEGACY: &str = "ENCRYPTED_MASTER_KEY";
const CIPHERTEXT_V2_PREFIX: &str = "v2";

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

pub fn decrypt_private_key(encrypted_key: &str) -> Result<String> {
    let master_key = load_master_key()?;
    decrypt_private_key_with_master_key(encrypted_key, &master_key)
}

pub fn encrypt_private_key(private_key: &str) -> Result<String> {
    let master_key = load_master_key()?;
    encrypt_private_key_with_master_key(private_key, &master_key)
}

fn load_master_key() -> Result<String> {
    let master_key = env::var(MASTER_KEY_ENV_PRIMARY)
        .or_else(|_| env::var(MASTER_KEY_ENV_LEGACY))
        .map_err(|_| {
            eyre!("{MASTER_KEY_ENV_PRIMARY} (preferred) or {MASTER_KEY_ENV_LEGACY} must be set")
        })?;

    if master_key.trim().is_empty() {
        return Err(eyre!(
            "{MASTER_KEY_ENV_PRIMARY}/{MASTER_KEY_ENV_LEGACY} must not be empty"
        ));
    }

    Ok(master_key)
}

fn derive_aead_key(master_key: &str) -> [u8; 32] {
    keccak256(master_key.as_bytes()).0
}

fn encrypt_private_key_with_master_key(private_key: &str, master_key: &str) -> Result<String> {
    let key_bytes = derive_aead_key(master_key);
    let cipher =
        Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| eyre!("invalid AES key: {e}"))?;
    let nonce_bytes = rand::random::<[u8; 12]>();
    let nonce = aes_gcm::Nonce::<U12>::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, private_key.as_bytes())
        .map_err(|e| eyre!("AES-GCM encryption failed: {e}"))?;

    Ok(format!(
        "{CIPHERTEXT_V2_PREFIX}:{}:{}",
        hex::encode(nonce_bytes),
        hex::encode(ciphertext)
    ))
}

fn decrypt_private_key_with_master_key(encrypted_key: &str, master_key: &str) -> Result<String> {
    if encrypted_key.starts_with(&format!("{CIPHERTEXT_V2_PREFIX}:")) {
        return decrypt_private_key_v2(encrypted_key, master_key);
    }

    decrypt_private_key_legacy_xor(encrypted_key, master_key)
}

fn decrypt_private_key_v2(encrypted_key: &str, master_key: &str) -> Result<String> {
    let mut parts = encrypted_key.split(':');
    let version = parts
        .next()
        .ok_or_else(|| eyre!("missing ciphertext version"))?;
    let nonce_hex = parts
        .next()
        .ok_or_else(|| eyre!("missing ciphertext nonce"))?;
    let ciphertext_hex = parts
        .next()
        .ok_or_else(|| eyre!("missing ciphertext payload"))?;
    if parts.next().is_some() {
        return Err(eyre!("invalid ciphertext format"));
    }
    if version != CIPHERTEXT_V2_PREFIX {
        return Err(eyre!("unsupported ciphertext version: {version}"));
    }

    let nonce_bytes_vec = hex::decode(nonce_hex).map_err(|e| eyre!("invalid nonce hex: {e}"))?;
    if nonce_bytes_vec.len() != 12 {
        return Err(eyre!(
            "invalid nonce length: expected 12 bytes, got {}",
            nonce_bytes_vec.len()
        ));
    }
    let nonce_bytes: [u8; 12] = nonce_bytes_vec
        .try_into()
        .map_err(|_| eyre!("invalid nonce length"))?;
    let ciphertext =
        hex::decode(ciphertext_hex).map_err(|e| eyre!("invalid ciphertext hex: {e}"))?;

    let key_bytes = derive_aead_key(master_key);
    let cipher =
        Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| eyre!("invalid AES key: {e}"))?;
    let nonce = aes_gcm::Nonce::<U12>::from(nonce_bytes);
    let plaintext = cipher
        .decrypt(&nonce, ciphertext.as_ref())
        .map_err(|e| eyre!("AES-GCM decryption failed: {e}"))?;

    String::from_utf8(plaintext).map_err(|e| eyre!("decrypted key is not valid UTF-8: {e}"))
}

fn decrypt_private_key_legacy_xor(encrypted_key: &str, master_key: &str) -> Result<String> {
    let encrypted_bytes =
        hex::decode(encrypted_key).map_err(|e| eyre!("invalid legacy ciphertext hex: {e}"))?;
    let master_key_bytes = master_key.as_bytes();
    let decrypted: Vec<u8> = encrypted_bytes
        .iter()
        .enumerate()
        .map(|(i, byte)| byte ^ master_key_bytes[i % master_key_bytes.len()])
        .collect();
    String::from_utf8(decrypted).map_err(|e| eyre!("legacy key is not valid UTF-8: {e}"))
}

#[cfg(test)]
fn encrypt_private_key_legacy_xor(private_key: &str, master_key: &str) -> String {
    let private_key_bytes = private_key.as_bytes();
    let master_key_bytes = master_key.as_bytes();
    let encrypted: Vec<u8> = private_key_bytes
        .iter()
        .enumerate()
        .map(|(i, byte)| byte ^ master_key_bytes[i % master_key_bytes.len()])
        .collect();
    hex::encode(encrypted)
}

/// Execute standard deposit operation
pub async fn execute_deposit(
    rpc_url: String,
    private_key: String,
    contract_address: String,
    order_id: String,
    amount: u64,
) -> Result<()> {
    println!("=== Executing Deposit Operation ===");

    // Parse parameters
    let contract_addr = parse_address(&contract_address)?;
    let order_id_bytes = parse_order_id(&order_id)?;
    let amount_u256 = U256::from(amount);

    // Create provider
    let provider = create_provider_with_wallet(&rpc_url, &private_key).await?;

    let signer = parse_private_key(&private_key)?;
    let wallet_address = signer.address();

    // Create contract instance
    let vault_contract = PaymentVault::new(contract_addr, &provider);

    // Get USDC token address
    let usdc_address = vault_contract.usdcToken().call().await?;
    let usdc_contract = USDC::new(usdc_address, &provider);

    println!("PaymentVault contract address: {contract_addr}");
    println!("USDC token address: {usdc_address}");
    println!("Order ID: {order_id}");
    println!("Deposit amount: {} USDC", format_usdc_amount(amount_u256));

    // Check USDC balance
    let balance = usdc_contract.balanceOf(wallet_address).call().await?;
    println!("Current USDC balance: {} USDC", format_usdc_amount(balance));

    if balance < amount_u256 {
        return Err(eyre!(
            "Insufficient USDC balance. Required: {} USDC, Current: {} USDC",
            format_usdc_amount(amount_u256),
            format_usdc_amount(balance)
        ));
    }

    // Check USDC allowance
    let allowance = usdc_contract
        .allowance(wallet_address, contract_addr)
        .call()
        .await?;
    println!("Current allowance: {} USDC", format_usdc_amount(allowance));

    if allowance < amount_u256 {
        println!("Insufficient allowance, approving...");

        // Approve tokens
        let approve_tx = usdc_contract
            .approve(contract_addr, amount_u256)
            .send()
            .await?;
        let approve_receipt = approve_tx.get_receipt().await?;

        println!(
            "Approve transaction hash: {}",
            approve_receipt.transaction_hash
        );
        println!(
            "Approve transaction status: {}",
            if approve_receipt.status() {
                "Success"
            } else {
                "Failed"
            }
        );

        if !approve_receipt.status() {
            return Err(eyre!("Approve transaction failed"));
        }
    }

    // Execute deposit
    println!("Executing deposit...");

    let deposit_tx = match vault_contract
        .deposit(order_id_bytes, amount_u256)
        .send()
        .await
    {
        Ok(tx) => tx,
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("already known") {
                println!(
                    "⚠️  Transaction already in mempool, this usually means it was already submitted successfully."
                );
                println!("Please check the blockchain explorer for the transaction status.");
                return Ok(());
            } else {
                return Err(e.into());
            }
        }
    };

    let deposit_receipt = deposit_tx.get_receipt().await?;

    println!(
        "Deposit transaction hash: {}",
        deposit_receipt.transaction_hash
    );
    println!(
        "Deposit transaction status: {}",
        if deposit_receipt.status() {
            "Success"
        } else {
            "Failed"
        }
    );

    if !deposit_receipt.status() {
        return Err(eyre!("Deposit transaction failed"));
    }

    // Parse event logs
    for log in deposit_receipt.inner.logs() {
        // Convert RPC log to primitive log
        let primitive_log = alloy::primitives::Log {
            address: log.address(),
            data: log.data().clone(),
        };

        if let Ok(event) = PaymentVault::DepositReceived::decode_log(&primitive_log) {
            println!("Deposit event details:");
            println!("  Payer: {}", event.payer);
            println!("  Order ID: {}", event.orderId);
            println!("  Amount: {} USDC", format_usdc_amount(event.amount));
        }
    }

    println!("✅ Deposit operation completed!");
    Ok(())
}

/// Execute withdraw operation (owner only)
pub async fn execute_withdraw(
    rpc_url: String,
    private_key: String,
    contract_address: String,
) -> Result<()> {
    println!("=== Executing Withdraw Operation ===");

    // Parse parameters
    let contract_addr = parse_address(&contract_address)?;

    // Create provider
    let provider = create_provider_with_wallet(&rpc_url, &private_key).await?;

    // Get wallet address
    let signer = parse_private_key(&private_key)?;
    let wallet_address = signer.address();
    println!("Wallet address: {wallet_address}");

    // Create contract instance
    let vault_contract = PaymentVault::new(contract_addr, &provider);

    // Get USDC token address
    let usdc_address = vault_contract.usdcToken().call().await?;
    let usdc_contract = USDC::new(usdc_address, &provider);

    println!("PaymentVault contract address: {contract_addr}");
    println!("USDC token address: {usdc_address}");

    // Check if owner
    let owner = vault_contract.owner().call().await?;
    if wallet_address != owner {
        return Err(eyre!(
            "Only the contract owner can execute withdraw. Current wallet: {wallet_address}, Owner: {owner}"
        ));
    }

    // Get contract balance
    let contract_balance = usdc_contract.balanceOf(contract_addr).call().await?;
    println!(
        "Contract USDC balance: {} USDC",
        format_usdc_amount(contract_balance)
    );

    if contract_balance == U256::ZERO {
        return Err(eyre!("No funds available for withdrawal in contract"));
    }

    // Execute withdraw
    println!("Executing withdraw...");

    let withdraw_tx = match vault_contract.withdraw().send().await {
        Ok(tx) => tx,
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("already known") {
                println!(
                    "⚠️  Transaction already in mempool, this usually means it was already submitted successfully."
                );
                println!("Please check the blockchain explorer for the transaction status.");
                return Ok(());
            } else {
                return Err(e.into());
            }
        }
    };

    let withdraw_receipt = withdraw_tx.get_receipt().await?;

    println!(
        "Withdraw transaction hash: {}",
        withdraw_receipt.transaction_hash
    );
    println!(
        "Withdraw transaction status: {}",
        if withdraw_receipt.status() {
            "Success"
        } else {
            "Failed"
        }
    );

    if !withdraw_receipt.status() {
        return Err(eyre!("Withdraw transaction failed"));
    }

    // Parse event logs
    for log in withdraw_receipt.inner.logs() {
        // Convert RPC log to primitive log
        let primitive_log = alloy::primitives::Log {
            address: log.address(),
            data: log.data().clone(),
        };

        if let Ok(event) = PaymentVault::WithdrawCompleted::decode_log(&primitive_log) {
            println!("Withdraw event details:");
            println!("  Owner: {}", event.owner);
            println!("  Amount: {} USDC", format_usdc_amount(event.amount));
        }
    }

    println!("✅ Withdraw operation completed!");
    Ok(())
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
    fn test_encrypt_decrypt_round_trip_v2() {
        let private_key = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let master_key = "unit-test-master-key";

        let encrypted = encrypt_private_key_with_master_key(private_key, master_key)
            .expect("v2 encryption should succeed");
        assert!(encrypted.starts_with("v2:"));

        let decrypted = decrypt_private_key_with_master_key(&encrypted, master_key)
            .expect("v2 decryption should succeed");
        assert_eq!(decrypted, private_key);
    }

    #[test]
    fn test_decrypt_legacy_xor_for_backward_compatibility() {
        let private_key = "0xabcdef";
        let master_key = "legacy-master-key";
        let legacy_ciphertext = encrypt_private_key_legacy_xor(private_key, master_key);

        let decrypted = decrypt_private_key_with_master_key(&legacy_ciphertext, master_key)
            .expect("legacy ciphertext should still decrypt");
        assert_eq!(decrypted, private_key);
    }
}
