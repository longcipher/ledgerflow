# LedgerFlow Facilitator Implementation Review

**Review Date**: 2025-01-16  
**Reviewed Against**: 
- `docs/scheme_exact_sui.md`
- `docs/scheme_exact_evm.md`

## Executive Summary

The facilitator implementation provides a good **foundation** for x402 payment processing but is **incomplete** for production use. While the architecture is sound and basic validation is implemented, critical features like cryptographic signature verification, on-chain balance checks, and proper settlement transactions are missing or incomplete.

**Status**: 🟡 **Foundation Complete, Production Features Missing**

---

## Sui Implementation Review (`scheme_exact_sui.md`)

### ✅ Correctly Implemented

#### 1. Signature Format Validation
```rust
// ✅ Base64 decoding, length checks, scheme flag validation
let sig_bytes = BASE64.decode(signature)?;
if sig_bytes.len() < 65 || sig_bytes.len() > 200 { ... }
let scheme_flag = sig_bytes[sig_bytes.len() - 1];
if scheme_flag > 3 { ... } // Ed25519=0, Secp256k1=1, Secp256r1=2, MultiSig=3
```
**Compliance**: Partial - detects format issues but doesn't verify cryptographically

#### 2. Intent Message Reconstruction
```rust
// ✅ Proper JSON structure with IntentScope::PersonalMessage
let auth_message = serde_json::json!({
    "intent": {
        "scope": "PersonalMessage",
        "version": "V0",
        "appId": "Sui"
    },
    "authorization": { /* ... */ }
});
```
**Compliance**: ✅ Matches specification exactly

#### 3. Timing Validation
```rust
// ✅ Checks validAfter and validBefore
let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
if now < payload.authorization.valid_after { return Err(...); }
if now > payload.authorization.valid_before { return Err(...); }
```
**Compliance**: ✅ Complete

#### 4. Nonce Replay Protection
```rust
// ✅ Thread-safe nonce tracking
let mut used_nonces = self.used_nonces.lock()?;
if used_nonces.contains(&nonce_str) {
    return Err(PaymentError::InvalidSignature("replay attack"));
}
used_nonces.insert(nonce_str);
```
**Compliance**: ⚠️ Works but not persistent (in-memory only)

#### 5. Amount & Recipient Validation
```rust
// ✅ Validates authorization amount and recipient address
if sui_payload.authorization.value.0 < requirements.max_amount_required.0 { ... }
if sui_payload.authorization.to != pay_to { ... }
```
**Compliance**: ✅ Complete

#### 6. Gas Budget Support
```rust
// ✅ Accepts optional gasBudget in payload
pub struct SuiPayload {
    pub signature: String,
    pub authorization: SuiPayloadAuthorization,
    pub gas_budget: Option<u64>, // ✅
}
```
**Compliance**: ✅ Matches Sui-specific requirements

---

### ❌ Missing Critical Features

#### 1. **Cryptographic Signature Verification** (CRITICAL)
**Scheme Requirement**: "Verify the signature against the message using the `from` address's public key"

**Current Implementation**:
```rust
// ❌ Only validates format, doesn't verify signature cryptographically
debug!("Basic signature validation passed"); // This is misleading!
Ok(())
```

**What's Needed**:
```rust
use sui_types::crypto::{Signature, PublicKey};
use blake2::Blake2b;

// Proper implementation should:
// 1. Hash the message with Blake2b-512
let message_hash = blake2b_hash(message_bytes);

// 2. Parse signature based on scheme flag
let signature = match scheme_flag {
    0 => Signature::Ed25519(parse_ed25519(&sig_bytes)?),
    1 => Signature::Secp256k1(parse_secp256k1(&sig_bytes)?),
    2 => Signature::Secp256r1(parse_secp256r1(&sig_bytes)?),
    3 => Signature::MultiSig(parse_multisig(&sig_bytes)?),
    _ => return Err(PaymentError::InvalidSignature),
};

// 3. Verify against expected signer
signature.verify(&message_hash, &expected_signer.into())?;
```

**Impact**: 🔴 **CRITICAL** - Current implementation accepts any signature

---

#### 2. **Balance Verification** (HIGH PRIORITY)
**Scheme Requirement**: "Query the payer's balance for the specified `coinType` and ensure balance covers `paymentRequirements.maxAmountRequired`"

**Current Implementation**: ❌ Not implemented

**What's Needed**:
```rust
async fn verify_balance(
    &self,
    payer: &SuiAddress,
    coin_type: &str,
    required_amount: u64,
) -> Result<(), PaymentError> {
    let client = self.get_client(network)?;
    
    // Query payer's coins of the specified type
    let coins = client
        .coin_read_api()
        .get_coins(*payer, Some(coin_type.to_string()), None, None)
        .await?;
    
    let total_balance: u64 = coins.data.iter().map(|c| c.balance).sum();
    
    if total_balance < required_amount {
        return Err(PaymentError::InsufficientFunds);
    }
    
    Ok(())
}
```

**Impact**: 🟠 **HIGH** - Cannot guarantee payer has funds

---

#### 3. **Coin Type Validation** (MEDIUM PRIORITY)
**Scheme Requirement**: "Verify the coin type exists and is active on the network"

**Current Implementation**: ❌ Not implemented

**What's Needed**:
```rust
async fn validate_coin_type(
    &self,
    coin_type: &str,
    network: Network,
) -> Result<(), PaymentError> {
    // Parse coin type (e.g., "0x2::sui::SUI")
    let parts: Vec<&str> = coin_type.split("::").collect();
    if parts.len() != 3 {
        return Err(PaymentError::InvalidAddress("Invalid coin type format"));
    }
    
    let package_id = ObjectID::from_hex_literal(parts[0])?;
    
    // Query package to verify coin type exists
    let client = self.get_client(&network)?;
    let _package = client
        .read_api()
        .get_object_with_options(package_id, /* options */)
        .await?;
    
    // TODO: Verify the coin module exists in package
    Ok(())
}
```

**Impact**: 🟡 **MEDIUM** - Could accept invalid coin types

---

#### 4. **Transaction Simulation (Dry-run)** (HIGH PRIORITY)
**Scheme Requirement**: "Dry-run the payment transaction to ensure it would succeed"

**Current Implementation**: ❌ Not implemented

**What's Needed**:
```rust
async fn simulate_payment(
    &self,
    sui_payload: &SuiPayload,
    vault_object_id: ObjectID,
) -> Result<(), PaymentError> {
    let client = self.sui_clients.get(&network)?;
    
    // Build transaction for PaymentVault::deposit
    let tx_builder = client.transaction_builder();
    let tx_data = tx_builder
        .move_call(
            vault_package_id,
            "payment_vault",
            "deposit_with_authorization",
            vec![], // type args
            vec![
                SuiJsonValue::from_object_id(vault_object_id),
                /* coin object, order_id, etc. */
            ],
            None, // signer (for simulation)
            sui_payload.gas_budget.unwrap_or(self.gas_budget),
            None,
        )
        .await?;
    
    // Dry-run the transaction
    let dry_run_result = client
        .read_api()
        .dry_run_transaction_block(tx_data.clone())
        .await?;
    
    // Check if simulation succeeded
    if let SuiExecutionStatus::Failure { error } = dry_run_result.effects.status() {
        return Err(PaymentError::TransactionExecutionError(error.to_string()));
    }
    
    Ok(())
}
```

**Impact**: 🟠 **HIGH** - Cannot predict transaction failures

---

#### 5. **Proper Settlement Implementation** (CRITICAL)
**Scheme Requirement**: Settlement should call PaymentVault contract, not do direct transfers

**Current Implementation**:
```rust
// ❌ Current implementation does a basic SUI transfer for testing
pub async fn execute_real_transfer(&self, amount: u64) -> Result<String, PaymentError> {
    // This is NOT the correct settlement flow!
    let tx_data = sui_client
        .transaction_builder()
        .transfer_sui(sender, coin_object_id, amount, recipient, None)
        .await?;
    // ...
}
```

**What's Needed**:
```rust
async fn settle_via_vault(
    &self,
    sui_payload: &SuiPayload,
    vault_config: &VaultConfig,
) -> Result<String, PaymentError> {
    let client = self.sui_clients.get(&payload.network)?;
    let keypair = self.get_keypair()?;
    let sender = SuiAddress::from(&keypair.public());
    
    // 1. Get payer's coin objects of the required type
    let coins = client
        .coin_read_api()
        .get_coins(
            sui_payload.authorization.from,
            Some(sui_payload.authorization.coin_type.clone()),
            None,
            None,
        )
        .await?;
    
    // 2. Select coin with sufficient balance (or merge coins)
    let coin_to_use = select_coin_for_payment(&coins, sui_payload.authorization.value.0)?;
    
    // 3. Build PaymentVault::deposit_with_authorization call
    let tx_data = client
        .transaction_builder()
        .move_call(
            vault_config.package_id,
            "payment_vault",
            "deposit_with_authorization",
            vec![sui_payload.authorization.coin_type.clone()], // type args: <T>
            vec![
                SuiJsonValue::from_object_id(vault_config.vault_object_id),
                SuiJsonValue::from_object_id(coin_to_use),
                SuiJsonValue::new(sui_payload.authorization.value.0.to_string().into())?,
                SuiJsonValue::new(order_id_bytes.into())?, // Derive from authorization
                SuiJsonValue::new(sui_payload.signature.clone().into())?, // Include authorization proof
            ],
            Some(sender), // Facilitator pays gas
            sui_payload.gas_budget.unwrap_or(self.gas_budget),
            None,
        )
        .await?;
    
    // 4. Sign and execute transaction
    let signature = keypair.sign(tx_data.digest().inner().as_ref());
    let signed_tx = Transaction::from_data(tx_data, vec![signature]);
    
    // 5. Submit to network
    let response = client
        .quorum_driver_api()
        .execute_transaction_block(
            signed_tx,
            SuiTransactionBlockResponseOptions::full_content(),
            Some(ExecuteTransactionRequestType::WaitForLocalExecution),
        )
        .await?;
    
    // 6. Verify success
    if let Some(effects) = &response.effects {
        match effects.status() {
            SuiExecutionStatus::Success => Ok(response.digest.to_string()),
            SuiExecutionStatus::Failure { error } => {
                Err(PaymentError::TransactionExecutionError(error.to_string()))
            }
        }
    } else {
        Err(PaymentError::TransactionExecutionError("Missing effects".to_string()))
    }
}
```

**Impact**: 🔴 **CRITICAL** - Settlement doesn't integrate with LedgerFlow system

---

#### 6. **Network-Specific Validation** (MEDIUM PRIORITY)
**Scheme Requirement**: "Ensure the authorization is for the correct Sui network (mainnet/testnet/devnet)"

**Current Implementation**: Partial - checks if network is supported, but doesn't validate signature was created for that network

**What's Needed**:
```rust
// The intent message should include network identifier
// Or validate against known network chain IDs
fn validate_network_match(
    &self,
    payload_network: Network,
    signature_network: Network,
) -> Result<(), PaymentError> {
    if payload_network != signature_network {
        return Err(PaymentError::InvalidSignature(
            format!("Network mismatch: payload={:?}, signature={:?}", 
                payload_network, signature_network)
        ));
    }
    Ok(())
}
```

**Impact**: 🟡 **MEDIUM** - Could allow cross-network replay

---

### ⚠️ Architecture Issues

#### 1. **In-Memory Nonce Storage**
```rust
// ⚠️ Nonces lost on restart, can't scale horizontally
used_nonces: Arc<Mutex<HashSet<String>>>
```

**Recommended**:
```rust
// Store nonces in PostgreSQL with expiration
async fn check_and_mark_nonce_used(
    &self,
    nonce: &HexEncodedNonce,
    valid_before: u64,
) -> Result<(), PaymentError> {
    sqlx::query!(
        "INSERT INTO used_nonces (nonce, expires_at) VALUES ($1, $2)
         ON CONFLICT (nonce) DO UPDATE SET nonce = EXCLUDED.nonce
         RETURNING nonce",
        hex::encode(nonce.0),
        i64::try_from(valid_before)?,
    )
    .fetch_one(&self.db)
    .await
    .map_err(|e| {
        if e.to_string().contains("duplicate key") {
            PaymentError::InvalidSignature("Nonce already used (replay attack)")
        } else {
            PaymentError::SuiError(e.to_string())
        }
    })?;
    
    Ok(())
}

// Cleanup expired nonces periodically
async fn cleanup_expired_nonces(&self) -> Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    sqlx::query!(
        "DELETE FROM used_nonces WHERE expires_at < $1",
        i64::try_from(now)?
    )
    .execute(&self.db)
    .await?;
    Ok(())
}
```

#### 2. **Configuration Management**
```rust
// ⚠️ Inconsistent environment variable names
std::env::var("SUI_PRIVATE_KEY")  // In execute_real_transfer
std::env::var("PRIVATE_KEY")       // In config.apply_env
```

**Recommended**: Use consistent naming:
```rust
// For multi-chain support, use network-specific keys:
SUI_TESTNET_PRIVATE_KEY
SUI_MAINNET_PRIVATE_KEY
EVM_BASE_PRIVATE_KEY
EVM_AVALANCHE_PRIVATE_KEY
```

---

## EVM Implementation Review (`scheme_exact_evm.md`)

### ✅ Correctly Implemented

#### 1. Network Mapping
```rust
// ✅ Correct chain IDs
Network::BaseSepolia => Ok(EvmChain::new(value, 84532)),
Network::Base => Ok(EvmChain::new(value, 8453)),
Network::XdcMainnet => Ok(EvmChain::new(value, 50)),
Network::AvalancheFuji => Ok(EvmChain::new(value, 43113)),
Network::Avalanche => Ok(EvmChain::new(value, 43114)),
```

#### 2. Basic Payload Validation
```rust
// ✅ Network, scheme, receiver checks
if payload.network != self.network() { ... }
if payload.scheme != requirements.scheme { ... }
if payload_to != requirements_to { ... }
```

#### 3. Timing with Grace Period
```rust
// ✅ 6-second grace buffer for expiration
if valid_before < now + 6 {
    return Err(PaymentError::InvalidTiming(...));
}
```

---

### ❌ Missing Critical Features

#### 1. **EIP-712 Signature Verification** (CRITICAL)
**Scheme Requirement**: "Verify the signature is valid" using EIP-712 typed structured data

**Current Implementation**: ❌ Not implemented at all

**What's Needed**:
```rust
use alloy::sol_types::{SolStruct, eip712_domain};
use alloy::primitives::{keccak256, Address, U256, B256};
use alloy::signers::{Signature, RecoveryId};

// Define EIP-712 domain
fn eip712_domain(contract_address: Address, chain_id: u64) -> Eip712Domain {
    eip712_domain! {
        name: "USD Coin",
        version: "2",
        chain_id: chain_id,
        verifying_contract: contract_address,
    }
}

// Define TransferWithAuthorization struct (EIP-3009)
#[derive(SolStruct)]
#[sol(name = "TransferWithAuthorization")]
struct TransferWithAuthorization {
    from: Address,
    to: Address,
    value: U256,
    valid_after: U256,
    valid_before: U256,
    nonce: B256,
}

async fn verify_eip712_signature(
    &self,
    evm_payload: &EvmPayload,
    usdc_contract: Address,
    chain_id: u64,
) -> Result<(), PaymentError> {
    // 1. Reconstruct the typed data
    let transfer = TransferWithAuthorization {
        from: evm_payload.authorization.from.0.into(),
        to: evm_payload.authorization.to.0.into(),
        value: U256::from(evm_payload.authorization.value.0),
        valid_after: U256::from(evm_payload.authorization.valid_after),
        valid_before: U256::from(evm_payload.authorization.valid_before),
        nonce: B256::from(evm_payload.authorization.nonce.0),
    };
    
    // 2. Compute EIP-712 hash
    let domain = eip712_domain(usdc_contract, chain_id);
    let struct_hash = transfer.eip712_hash_struct();
    let domain_separator = domain.hash_struct();
    
    let digest = keccak256([
        &[0x19, 0x01][..],
        domain_separator.as_slice(),
        struct_hash.as_slice(),
    ].concat());
    
    // 3. Recover signer from signature
    let sig_bytes = evm_payload.signature.0;
    let signature = Signature::from_bytes_and_parity(
        &sig_bytes[0..64],
        RecoveryId::try_from(sig_bytes[64])?,
    )?;
    
    let recovered_address = signature.recover_address_from_prehash(&digest)?;
    
    // 4. Verify recovered address matches expected from
    if recovered_address != evm_payload.authorization.from.0 {
        return Err(PaymentError::InvalidSignature(
            format!("Signature verification failed: expected {}, got {}",
                evm_payload.authorization.from.0, recovered_address)
        ));
    }
    
    Ok(())
}
```

**Impact**: 🔴 **CRITICAL** - Currently accepts any signature without verification

---

#### 2. **EIP-3009 Contract Integration** (CRITICAL)
**Scheme Requirement**: "Call `transferWithAuthorization` function on the EIP-3009 compliant contract"

**Current Implementation**: 
```rust
// ❌ Just returns fake transaction hash
let fake_tx_hash = [0u8; 32];
Ok(SettleResponse { transaction: Some(TransactionHash::Evm(fake_tx_hash)), ... })
```

**What's Needed**:
```rust
use alloy::contract::Contract;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::network::EthereumWallet;
use alloy::sol;

// Define USDC contract ABI for EIP-3009
sol! {
    #[sol(rpc)]
    interface IUSDC {
        function transferWithAuthorization(
            address from,
            address to,
            uint256 value,
            uint256 validAfter,
            uint256 validBefore,
            bytes32 nonce,
            bytes memory signature
        ) external;
        
        function authorizationState(
            address authorizer,
            bytes32 nonce
        ) external view returns (bool);
    }
}

async fn settle_eip3009(
    &self,
    evm_payload: &EvmPayload,
    usdc_address: Address,
    rpc_url: &str,
) -> Result<[u8; 32], PaymentError> {
    // 1. Setup provider and signer
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(rpc_url.parse()?);
    
    let wallet = self.get_evm_wallet()?;
    let provider_with_wallet = provider.with_wallet(wallet);
    
    // 2. Create contract instance
    let usdc = IUSDC::new(usdc_address, &provider_with_wallet);
    
    // 3. Check nonce hasn't been used
    let nonce_used = usdc
        .authorizationState(
            evm_payload.authorization.from.0,
            B256::from(evm_payload.authorization.nonce.0),
        )
        .call()
        .await?
        ._0;
    
    if nonce_used {
        return Err(PaymentError::InvalidSignature("Nonce already used".into()));
    }
    
    // 4. Call transferWithAuthorization
    let tx_builder = usdc.transferWithAuthorization(
        evm_payload.authorization.from.0,
        evm_payload.authorization.to.0,
        U256::from(evm_payload.authorization.value.0),
        U256::from(evm_payload.authorization.valid_after),
        U256::from(evm_payload.authorization.valid_before),
        B256::from(evm_payload.authorization.nonce.0),
        evm_payload.signature.0.to_vec().into(),
    );
    
    // 5. Send transaction and wait for confirmation
    let pending_tx = tx_builder.send().await?;
    let receipt = pending_tx.get_receipt().await?;
    
    if !receipt.status() {
        return Err(PaymentError::TransactionExecutionError(
            "Transaction reverted".into()
        ));
    }
    
    // 6. Return transaction hash
    Ok(receipt.transaction_hash.0)
}
```

**Impact**: 🔴 **CRITICAL** - No actual settlement happening

---

#### 3. **Balance Checking** (HIGH PRIORITY)
**Scheme Requirement**: "Verify the client has enough of the asset (ERC20 token)"

**Current Implementation**: ❌ Not implemented

**What's Needed**:
```rust
sol! {
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
    }
}

async fn verify_erc20_balance(
    &self,
    payer: Address,
    token_address: Address,
    required_amount: u64,
    rpc_url: &str,
) -> Result<(), PaymentError> {
    let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);
    let token = IERC20::new(token_address, &provider);
    
    let balance = token.balanceOf(payer).call().await?._0;
    
    if balance < U256::from(required_amount) {
        return Err(PaymentError::InsufficientFunds);
    }
    
    Ok(())
}
```

**Impact**: 🟠 **HIGH** - Cannot guarantee payer has funds

---

#### 4. **Transaction Simulation** (HIGH PRIORITY)
**Scheme Requirement**: "Simulate the `transferWithAuthorization` to ensure the transaction would succeed"

**Current Implementation**: ❌ Not implemented

**What's Needed**:
```rust
async fn simulate_transfer_with_authorization(
    &self,
    evm_payload: &EvmPayload,
    usdc_address: Address,
    rpc_url: &str,
) -> Result<(), PaymentError> {
    let provider = ProviderBuilder::new().on_http(rpc_url.parse()?);
    let usdc = IUSDC::new(usdc_address, &provider);
    
    // Simulate the call (eth_call)
    let result = usdc
        .transferWithAuthorization(
            evm_payload.authorization.from.0,
            evm_payload.authorization.to.0,
            U256::from(evm_payload.authorization.value.0),
            U256::from(evm_payload.authorization.valid_after),
            U256::from(evm_payload.authorization.valid_before),
            B256::from(evm_payload.authorization.nonce.0),
            evm_payload.signature.0.to_vec().into(),
        )
        .call()
        .await;
    
    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(PaymentError::TransactionExecutionError(
            format!("Simulation failed: {}", e)
        )),
    }
}
```

**Impact**: 🟠 **HIGH** - Cannot predict transaction failures

---

#### 5. **RPC Client Initialization** (CRITICAL)
**Current Implementation**: 
```rust
// ❌ No actual RPC clients created
providers: HashMap<Network, EvmProvider>, // EvmProvider has no RPC client!
```

**What's Needed**:
```rust
#[derive(Clone)]
pub struct EvmProvider {
    chain: EvmChain,
    provider: Arc<dyn Provider>, // ✅ Add actual RPC provider
    usdc_address: Address,
    vault_address: Option<Address>,
}

impl EvmProvider {
    pub async fn new(
        network: Network,
        rpc_url: &str,
        usdc_address: Address,
        vault_address: Option<Address>,
    ) -> Result<Self, PaymentError> {
        let chain = EvmChain::try_from(network)?;
        
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .on_http(rpc_url.parse()?)
            .into();
        
        Ok(Self {
            chain,
            provider,
            usdc_address,
            vault_address,
        })
    }
}
```

**Impact**: 🔴 **CRITICAL** - No blockchain interaction possible

---

### ⚠️ Architecture Issues

#### 1. **Missing Gas Management**
```rust
// Configuration has these fields but they're not used
pub gas_limit: Option<u64>,
pub gas_price: Option<u64>,
```

**Recommended**:
```rust
async fn estimate_gas_for_transfer(
    &self,
    evm_payload: &EvmPayload,
) -> Result<u128, PaymentError> {
    let provider = &self.provider;
    
    // Build the transaction
    let tx = usdc.transferWithAuthorization(...).into_transaction_request();
    
    // Estimate gas
    let gas_estimate = provider.estimate_gas(&tx).await?;
    
    // Add 20% buffer
    Ok(gas_estimate * 120 / 100)
}
```

#### 2. **No Wallet Management**
The facilitator needs to sign settlement transactions but has no wallet infrastructure:

```rust
// ❌ Missing
fn get_evm_wallet(&self) -> Result<EthereumWallet, PaymentError> {
    let private_key = std::env::var("EVM_PRIVATE_KEY")
        .map_err(|_| PaymentError::TransactionExecutionError("EVM_PRIVATE_KEY not set"))?;
    
    let signer = LocalSigner::from_str(&private_key)?;
    Ok(EthereumWallet::from(signer))
}
```

---

## Configuration Issues

### Current State
```rust
// config.rs has inconsistent key handling
if let Ok(env_private_key) = std::env::var("SUI_PRIVATE_KEY") {
    unsafe { std::env::set_var("PRIVATE_KEY", env_private_key); }
}
```

### Problems:
1. ❌ No separation between Sui and EVM keys
2. ❌ Unsafe blocks for env var manipulation
3. ❌ Config struct doesn't support per-network private keys
4. ❌ No validation of key formats

### Recommended Configuration:
```toml
# config.toml
[server]
host = "0.0.0.0"
port = 3402

# Sui Networks
[sui.testnet]
grpc_url = "https://fullnode.testnet.sui.io:443"
usdc_package_id = "0x..."
vault_package_id = "0x..."
# Private key should ONLY be in environment variable
# private_key_env = "SUI_TESTNET_PRIVATE_KEY"

[sui.mainnet]
grpc_url = "https://fullnode.mainnet.sui.io:443"
usdc_package_id = "0x..."
vault_package_id = "0x..."

# EVM Networks
[evm.base_sepolia]
chain_id = 84532
rpc_url = "https://sepolia.base.org"
usdc_address = "0x036CbD53842c5426634e7929541eC2318f3dCF7e"
vault_address = "0x..."
# private_key_env = "EVM_BASE_SEPOLIA_PRIVATE_KEY"

[evm.base]
chain_id = 8453
rpc_url = "https://mainnet.base.org"
usdc_address = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
vault_address = "0x..."

# Transaction Settings
[transaction]
sui_gas_budget = 100_000_000  # 0.1 SUI
evm_gas_limit = 200_000
evm_gas_price_multiplier = 1.2  # 20% above estimated
```

---

## Priority Implementation Roadmap

### Phase 1: Critical Security (Week 1-2)
1. 🔴 **Sui cryptographic signature verification** 
   - Implement Blake2b hashing
   - Add signature scheme parsing (Ed25519/Secp256k1/Secp256r1/MultiSig)
   - Use Sui SDK's crypto verification

2. 🔴 **EVM EIP-712 signature verification**
   - Implement EIP-712 typed data hashing
   - Add signature recovery
   - Validate against expected signer

3. 🔴 **Persistent nonce storage**
   - PostgreSQL table for used nonces
   - Expiration cleanup job
   - Horizontal scaling support

### Phase 2: Settlement Integration (Week 3-4)
4. 🔴 **Sui PaymentVault integration**
   - Implement `deposit_with_authorization` calls
   - Add order_id derivation logic
   - Proper gas budget management

5. 🔴 **EVM EIP-3009 integration**
   - Implement `transferWithAuthorization` calls
   - Add wallet management for facilitator
   - Gas estimation and retry logic

### Phase 3: Validation & Safety (Week 5-6)
6. 🟠 **Balance verification**
   - Sui: Query coin balances via RPC
   - EVM: Query ERC20 balanceOf

7. 🟠 **Transaction simulation**
   - Sui: Dry-run transactions
   - EVM: eth_call simulation

8. 🟡 **Coin/Token type validation**
   - Sui: Validate coin type exists
   - EVM: Validate ERC20 contract

### Phase 4: Production Readiness (Week 7-8)
9. ⚪ Configuration improvements
   - Structured config with per-network settings
   - Environment variable validation
   - Secret management integration

10. ⚪ Monitoring & Observability
    - Transaction success/failure metrics
    - Settlement latency tracking
    - Error rate monitoring

11. ⚪ Testing
    - Integration tests with real testnets
    - Load testing
    - Failure scenario testing

---

## Testing Gaps

### Current Test Coverage
- ✅ Basic signature format validation (21 unit tests)
- ✅ Business logic validation (timing, amounts, nonces)
- ✅ HTTP endpoint integration (4 tests)
- ✅ Concurrent operation safety

### Missing Tests
- ❌ Cryptographic signature verification with real keys
- ❌ On-chain balance checks
- ❌ Transaction simulation success/failure
- ❌ Settlement transaction execution
- ❌ Multi-network scenarios
- ❌ Nonce persistence and cleanup
- ❌ Gas estimation accuracy
- ❌ Error recovery and retries

---

## Summary

The facilitator has a **solid architectural foundation** with proper separation of concerns, good error handling patterns, and extensible design. However, it is **not production-ready** due to missing critical features:

### Critical Gaps (Must Fix Before Production):
1. ✅ Architecture & Structure ← **Done**
2. ✅ Basic Validation Logic ← **Done**
3. ❌ **Cryptographic Verification** ← **CRITICAL MISSING**
4. ❌ **On-Chain Settlement** ← **CRITICAL MISSING**
5. ❌ **Persistent Nonce Storage** ← **CRITICAL MISSING**
6. ❌ **Balance & Simulation Checks** ← **HIGH PRIORITY MISSING**

### Recommended Next Steps:
1. Implement cryptographic signature verification (both Sui and EVM)
2. Add RPC client initialization with proper error handling
3. Implement proper settlement transactions (PaymentVault/EIP-3009)
4. Add PostgreSQL for nonce persistence
5. Implement balance verification and transaction simulation
6. Add comprehensive integration tests with real testnets
7. Implement monitoring and alerting

**Estimated Effort**: 6-8 weeks for production-ready implementation with proper testing.
