# LedgerFlow Vault - Aptos Move Module

LedgerFlow Vault Aptos is the Aptos blockchain implementation of the LedgerFlow payment gateway, providing secure and efficient on-chain fund custody and payment processing functionality. This module is developed using the Move language, taking full advantage of Aptos blockchain's resource model and security features.

## 🎯 Core Features

- **Non-Custodial Vault**: Serves as the sole entry point and vault for funds, receiving and storing all USDC payments
- **Order Association**: Each deposit is associated with a unique `order_id`, enabling on-chain and off-chain data synchronization
- **Secure Transfers**: Safe fund transfers based on the Aptos Coin standard
- **Event Triggering**: Emits `DepositReceived` events for off-chain indexer monitoring
- **Access Control**: Only allows contract owner to withdraw funds to specified addresses
- **Token Recovery**: Emergency function to recover accidentally sent tokens to the contract
- **Upgrade Support**: Supports contract upgrades to adapt to business development needs

## 🏗️ Core Algorithm

### Order ID Generation

To ensure `order_id` uniqueness, collision prevention, and unpredictability, uses the same algorithm concept as the EVM version:

```move
// In Move, we use the std::hash module
order_id = std::hash::sha3_256(broker_id || account_id || order_id_num)
```

- `broker_id`: Unique identifier for merchant/platform
- `account_id`: Unique identifier for paying user
- `order_id_num`: Order sequence number for that account

## 🌟 Aptos Architecture Advantages

### Resource Model Benefits

Unlike EVM's account model, Aptos uses a resource model, providing the following advantages:

1. **Resource Ownership**: Resources can only be accessed and modified by their owners
2. **Linear Types**: Resources cannot be copied or dropped, preventing double spending
3. **Composability**: Resources can be safely combined and transferred
4. **Parallel Execution**: Supports higher transaction throughput

### Security Features

- **Move Verifier**: Statically verifies contract security before deployment
- **Resource Safety**: Prevents resource leaks and reuse
- **Type Safety**: Strong type system prevents type confusion attacks
- **Access Control**: Fine-grained permission control through capability patterns

## 📋 Smart Contract Interface

### Core Structures

```move
/// Vault resource stored under the contract publisher's account
struct PaymentVault has key {
    /// USDC token storage
    usdc_store: Coin<USDC>,
    /// Owner capability
    owner_cap: OwnerCapability,
    /// Deposit counter
    deposit_count: u64,
}

/// Owner capability for permission control
struct OwnerCapability has key, store {
    /// Capability holder address
    owner: address,
}
```

### Public Functions

#### `initialize(account: &signer)`

Initialize the payment vault, creating necessary resource structures.

**Parameters:**

- `account`: Signer reference of the contract publisher

**Permissions:** Can only be called by the contract publisher

#### `deposit(payer: &signer, order_id: vector<u8>, amount: u64)`

Standard deposit function to deposit specified amount of USDC into the vault.

**Parameters:**

- `payer`: Signer reference of the payer
- `order_id`: Unique order identifier (32 bytes)
- `amount`: Deposit amount (in USDC smallest units)

**Preconditions:**

- Payer account must have sufficient USDC balance
- `amount > 0`
- `order_id` length must be 32 bytes

#### `withdraw(owner: &signer, recipient: address, amount: u64)`

Owner withdraws specified amount of USDC to specified address.

**Parameters:**

- `owner`: Owner's signer reference
- `recipient`: Recipient address
- `amount`: Withdrawal amount

**Permissions:** Can only be called by contract owner

#### `withdraw_all(owner: &signer, recipient: address)`

Owner withdraws all USDC from vault to specified address.

**Parameters:**

- `owner`: Owner's signer reference
- `recipient`: Recipient address

**Permissions:** Can only be called by contract owner

#### `transfer_ownership(current_owner: &signer, new_owner: address)`

Transfer contract ownership to new address.

**Parameters:**

- `current_owner`: Current owner's signer reference
- `new_owner`: New owner address

**Permissions:** Can only be called by current owner

### View Functions

#### `get_balance(): u64`

Get the current USDC balance in the vault.

**Return Value:** Current USDC balance

#### `get_owner(): address`

Get the current contract owner address.

**Return Value:** Owner address

#### `get_deposit_count(): u64`

Get the cumulative number of deposits.

**Return Value:** Deposit count

## 📡 Event System

### DepositReceived

Event emitted when a deposit is successful.

```move
struct DepositReceived has drop, store {
    payer: address,
    order_id: vector<u8>,
    amount: u64,
    timestamp: u64,
    deposit_index: u64,
}
```

### WithdrawCompleted

Event emitted when a withdrawal is successful.

```move
struct WithdrawCompleted has drop, store {
    owner: address,
    recipient: address,
    amount: u64,
    timestamp: u64,
}
```

### OwnershipTransferred

Event emitted when ownership is transferred.

```move
struct OwnershipTransferred has drop, store {
    previous_owner: address,
    new_owner: address,
    timestamp: u64,
}
```

## 🔒 Security Considerations

### Access Control

- **Owner Verification**: Uses `OwnerCapability` resource to ensure only legitimate owners can execute management operations
- **Resource Protection**: Vault resources are stored under the contract account, preventing external direct access
- **Parameter Validation**: All functions perform strict parameter validation

### Resource Safety

- **Linear Types**: Coin resources' linear types ensure tokens cannot be copied or accidentally destroyed
- **Atomicity**: All operations are atomic, either all succeed or all fail
- **Reentrancy Prevention**: Move language naturally prevents reentrancy attacks

### Error Handling

```move
/// Error code definitions
const E_NOT_INITIALIZED: u64 = 1;
const E_ALREADY_INITIALIZED: u64 = 2;
const E_NOT_OWNER: u64 = 3;
const E_INSUFFICIENT_BALANCE: u64 = 4;
const E_INVALID_AMOUNT: u64 = 5;
const E_INVALID_ORDER_ID: u64 = 6;
const E_INVALID_ADDRESS: u64 = 7;
```

## 🚀 Deployment & Upgrades

### Deployment Process

1. **Compile contracts**

   ```bash
   aptos move compile
   ```

2. **Run tests**

   ```bash
   aptos move test
   ```

3. **Deploy to testnet**

   ```bash
   aptos move publish --profile testnet
   ```

4. **Deploy to mainnet**

   ```bash
   aptos move publish --profile mainnet
   ```

### Upgrade Mechanism

Aptos supports contract upgrades, requiring:

1. **Compatibility Policy**: Set appropriate upgrade policy

   ```toml
   [package]
   upgrade_policy = "compatible"  # or "immutable"
   ```

2. **Upgrade Command**

   ```bash
   aptos move upgrade-package --profile mainnet
   ```

## 🌐 Multi-Chain Support

### Network Configuration

Supports deployment to multiple Aptos networks:

- **Mainnet**: <https://fullnode.mainnet.aptoslabs.com>
- **Testnet**: <https://fullnode.testnet.aptoslabs.com>
- **Devnet**: <https://fullnode.devnet.aptoslabs.com>

### Address Configuration

```toml
[addresses]
ledgerflow_vault = "0x1"  # Replace with actual address when deploying

[dev-addresses]
ledgerflow_vault = "0x1"
```

## 📊 Comparison with EVM Version

| Feature | EVM (Solidity) | Aptos (Move) |
|---------|----------------|--------------|
| Programming Model | Account Model | Resource Model |
| Type Safety | Runtime Checks | Compile-time Verification |
| Parallelism | Limited | Native Support |
| Upgrade Mechanism | UUPS Proxy | Package Upgrade |
| Access Control | Ownable | Capability |
| Event System | Logs | Structured Events |
| Gas Model | Execution Complexity | Resource Usage |

## 🧪 Testing Strategy

### Unit Tests

- Initialization functionality tests
- Deposit functionality tests
- Withdrawal functionality tests
- Access control tests
- Error condition tests

### Integration Tests

- End-to-end deposit flows
- Multi-user concurrent tests
- Event emission verification
- Upgrade compatibility tests

### Performance Tests

- Large-scale deposit performance
- Concurrent transaction processing
- Gas consumption analysis

## 📁 Project Structure

```text
ledgerflow-vault-aptos/
├── sources/                     # Move source code
│   ├── payment_vault.move      # Main vault contract
│   └── usdc.move              # USDC token definition (for testing)
├── tests/                      # Test files
│   ├── payment_vault_tests.move
│   └── integration_tests.move
├── scripts/                    # Deployment scripts
│   ├── deploy.move
│   └── initialize.move
├── Move.toml                   # Project configuration file
├── README.md                   # This document
└── .gitignore                  # Git ignore file
```

## 🔗 Related Documentation

- [Aptos Move Programming Guide](https://aptos.dev/move/move-book/)
- [Aptos Coin Standard](https://aptos.dev/standards/coin/)
- [LedgerFlow Architecture Documentation](../README.md)
- [Deployment Guide](docs/deployment.md)
- [Testing Guide](docs/testing.md)

## 📝 Development Roadmap

### Phase 1: Core Features ✅

- [x] Basic contract structure design
- [x] Deposit function implementation
- [x] Withdrawal function implementation
- [x] Event system

### Phase 2: Security Enhancement 🚧

- [ ] Complete test coverage
- [ ] Security audit
- [ ] Performance optimization
- [ ] Documentation improvement

### Phase 3: Production Ready 📅

- [ ] Mainnet deployment
- [ ] Monitoring system
- [ ] Upgrade mechanism testing
- [ ] Cross-chain bridge support

## 🤝 Contributing

1. Fork this repository
2. Create feature branch: `git checkout -b feature/amazing-feature`
3. Commit changes: `git commit -m 'Add amazing feature'`
4. Push to branch: `git push origin feature/amazing-feature`
5. Open Pull Request

## 📄 License

This project is licensed under the Apache-2.0 License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Aptos Labs](https://aptoslabs.com/) - For providing excellent blockchain infrastructure
- [Move Language Team](https://github.com/move-language/move) - For creating a secure smart contract language
- LedgerFlow Community - For continuous support and feedback
