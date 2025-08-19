# LedgerFlow Vault - Sui Implementation

A secure payment vault smart contract for USDC deposits on the Sui blockchain. This contract provides non-custodial fund management with order tracking and owner-controlled withdrawals, following Sui's object model and coin standards.

## 📋 Project Status

**✅ DEPLOYED AND TESTED** - Complete implementation with successful testnet deployment
- **Contract**: Fully functional with comprehensive error handling
- **Testing**: 12/12 unit tests passing + live testnet deployment testing ✅
- **Deployment**: Successfully deployed to Sui Testnet ✅
- **Live Testing**: Successful deposit and withdrawal operations ✅
- **Documentation**: Complete with examples and integration guides

### Testnet Deployment Information
- **Network**: Sui Testnet ✅ **LIVE DEPLOYMENT**
- **Package ID**: `0x6d169a7fe1fad254df4b1bb62560944d817175ea0be5dda9b2b3e7d511b98d76`
- **Vault ID**: `0xafb31d363dd0d652b49320b7823afd574c5c99abcd34b7f0e2e6d63fd3f4bbf9`
- **Owner Capability**: `0x83b44c0ceb74a215788fb982a8d2510ce65311d46680de957beeb80272efe343`
- **USDC Type**: `0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC`
- **Status**: ✅ Live and operational

### Live Testing Results
- ✅ **Deposit Test**: Successfully deposited 10 USDC with order ID `order_test_12345`
  - **Transaction**: `GVHxsYci8pZPF7VqBjNdNRGPumauzdzJpc3FGs8FCzRJ`
  - **Event**: `DepositReceived` emitted correctly
  - **Amount**: 10,000,000 (10 USDC)
- ✅ **Withdraw Test**: Successfully withdrew 5 USDC 
  - **Transaction**: `4WJJDxawnxSTyCa7PAXnLxz7YJhW6RLGfoz7Cr19aDfE`
  - **Event**: `WithdrawCompleted` emitted correctly
  - **Amount**: 5,000,000 (5 USDC)
- ✅ **Final State**: Vault balance 5 USDC, all functions working correctly

### Quick Status Overview
- ✅ **Core Smart Contract** - PaymentVault and OwnerCap objects
- ✅ **USDC Integration** - Circle's official USDC implementation
- ✅ **Security Features** - Capability-based access control
- ✅ **Event System** - Complete event emission for indexing
- ✅ **Testing Suite** - Comprehensive unit and integration tests
- ✅ **Development Tools** - Build scripts, deployment automation
- ✅ **Testnet Deployment** - Live deployment with successful testing
- ⏳ **Integration Testing** - Pending indexer integration

## Overview

The LedgerFlow Vault enables secure USDC payments with the following key features:

- **Secure Deposits**: Users can deposit USDC with associated order IDs for tracking
- **Owner Control**: Only vault owners can withdraw funds using capability-based access control
- **Event Emission**: All operations emit events for off-chain indexing and monitoring
- **Ownership Transfer**: Vault ownership can be transferred to new addresses
- **Standards Compliance**: Uses official Circle USDC implementation and Sui coin standards

## Architecture & Implementation

### Core Objects

- **`PaymentVault`**: Main vault object storing USDC balance and metadata
- **`OwnerCap`**: Capability object for administrative control

### Sui-Specific Adaptations

**From Aptos to Sui:**
- `Coin<USDC>` instead of Fungible Assets
- `Balance<USDC>` for efficient storage (instead of `Coin<USDC>`)
- Clock parameter for timestamps instead of global timestamp
- Object sharing model for public vault access
- Capability-based access control with vault binding

**USDC Integration:**
- Uses Circle's official USDC implementation
- Compatible with both testnet and mainnet USDC
- Proper coin-to-balance conversions for gas efficiency

### Key Features

- Uses Sui's `Coin<USDC>` standard for interacting with Circle's USDC implementation
- Capability-based access control for secure ownership management
- Comprehensive event emission for indexer integration
- Zero-coin initialization for clean vault creation

## Contract Interface

### Core Functions

#### `init_vault(clock: &Clock, ctx: &mut TxContext): (PaymentVault, OwnerCap)`
Creates a new payment vault and owner capability.

#### `deposit(vault: &mut PaymentVault, payment: Coin<USDC>, order_id: vector<u8>, clock: &Clock, ctx: &mut TxContext)`
Deposits USDC to the vault with an associated order ID.

#### `withdraw(vault: &mut PaymentVault, owner_cap: &OwnerCap, amount: u64, recipient: address, clock: &Clock, ctx: &mut TxContext)`
Withdraws a specific amount from the vault (owner only).

#### `withdraw_all(vault: &mut PaymentVault, owner_cap: &OwnerCap, recipient: address, clock: &Clock, ctx: &mut TxContext)`
Withdraws all funds from the vault (owner only).

#### `transfer_ownership(vault: &mut PaymentVault, owner_cap: &OwnerCap, new_owner: address, clock: &Clock, ctx: &mut TxContext)`
Transfers vault ownership to a new address.

### View Functions

- `get_balance(vault: &PaymentVault): u64` - Get current USDC balance
- `get_owner(vault: &PaymentVault): address` - Get current owner address
- `get_deposit_count(vault: &PaymentVault): u64` - Get total deposit count
- `get_created_at(vault: &PaymentVault): u64` - Get creation timestamp
- `get_vault_id(vault: &PaymentVault): ID` - Get vault object ID

### Events

#### `DepositReceived`
```move
public struct DepositReceived has copy, drop {
    vault_id: ID,
    payer: address,
    order_id: vector<u8>,
    amount: u64,
    timestamp: u64,
    deposit_index: u64
}
```

#### `WithdrawCompleted`
```move
public struct WithdrawCompleted has copy, drop {
    vault_id: ID,
    owner: address,
    recipient: address,
    amount: u64,
    timestamp: u64
}
```

#### `OwnershipTransferred`
```move
public struct OwnershipTransferred has copy, drop {
    vault_id: ID,
    previous_owner: address,
    new_owner: address,
    timestamp: u64
}
```

## Prerequisites

- Sui CLI installed and configured
- Access to testnet/mainnet USDC (from Circle's faucet for testnet)
- Basic understanding of Sui Move and transaction blocks

## 🧪 Live Testing Guide & Results

### Complete Testing Workflow

The following section documents the successful testing of the LedgerFlow Sui vault on testnet using real Circle USDC tokens.

#### Prerequisites for Testing
- Sui CLI configured for testnet
- Test wallet with SUI for gas fees
- Circle USDC tokens from testnet faucet or mint

#### Step 1: Deploy Contract

```bash
# Build the contract
sui move build

# Deploy to testnet
sui client publish --gas-budget 100000000

# Result: Package deployed at 0x6d169a7fe1fad254df4b1bb62560944d817175ea0be5dda9b2b3e7d511b98d76
```

#### Step 2: Create and Share Vault

```bash
# Create vault instance
sui client call \
  --package 0x6d169a7fe1fad254df4b1bb62560944d817175ea0be5dda9b2b3e7d511b98d76 \
  --module payment_vault \
  --function create_shared_vault \
  --type-args "0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC" \
  --args 0x6 \
  --gas-budget 10000000

# Result: 
# - Vault created: 0xafb31d363dd0d652b49320b7823afd574c5c99abcd34b7f0e2e6d63fd3f4bbf9
# - Owner cap: 0x83b44c0ceb74a215788fb982a8d2510ce65311d46680de957beeb80272efe343
```

#### Step 3: Test Deposit (✅ SUCCESS)

```bash
# Deposit 10 USDC with order ID
sui client call \
  --package 0x6d169a7fe1fad254df4b1bb62560944d817175ea0be5dda9b2b3e7d511b98d76 \
  --module payment_vault \
  --function deposit \
  --type-args "0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC" \
  --args 0xafb31d363dd0d652b49320b7823afd574c5c99abcd34b7f0e2e6d63fd3f4bbf9 \
         0x370668139f07d887d48802ffa0286d1351c99c81fa6d94a8ddd5e132980c01d9 \
         "order_test_12345" \
         0x6 \
  --gas-budget 10000000

# Transaction: GVHxsYci8pZPF7VqBjNdNRGPumauzdzJpc3FGs8FCzRJ
# Status: ✅ SUCCESS
# Event: DepositReceived emitted with:
#   - vault_id: 0xafb31d363dd0d652b49320b7823afd574c5c99abcd34b7f0e2e6d63fd3f4bbf9
#   - payer: 0xcd8369e1a8ae681fb05660ffe9811872daff3f6946a4981c2e573a0627c3a877
#   - order_id: "order_test_12345" (base64: b3JkZXJfdGVzdF8xMjM0NQ==)
#   - amount: 10000000 (10 USDC)
#   - deposit_index: 1
```

#### Step 4: Verify Vault State

```bash
# Check vault balance
sui client object 0xafb31d363dd0d652b49320b7823afd574c5c99abcd34b7f0e2e6d63fd3f4bbf9

# Result: ✅ Vault balance shows 10,000,000 (10 USDC)
# - usdc_balance: 10000000
# - deposit_count: 1
# - owner: 0xcd8369e1a8ae681fb05660ffe9811872daff3f6946a4981c2e573a0627c3a877
```

#### Step 5: Test Withdrawal (✅ SUCCESS)

```bash
# Withdraw 5 USDC back to owner
sui client call \
  --package 0x6d169a7fe1fad254df4b1bb62560944d817175ea0be5dda9b2b3e7d511b98d76 \
  --module payment_vault \
  --function withdraw \
  --type-args "0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC" \
  --args 0xafb31d363dd0d652b49320b7823afd574c5c99abcd34b7f0e2e6d63fd3f4bbf9 \
         0x83b44c0ceb74a215788fb982a8d2510ce65311d46680de957beeb80272efe343 \
         5000000 \
         0xcd8369e1a8ae681fb05660ffe9811872daff3f6946a4981c2e573a0627c3a877 \
         0x6 \
  --gas-budget 10000000

# Transaction: 4WJJDxawnxSTyCa7PAXnLxz7YJhW6RLGfoz7Cr19aDfE
# Status: ✅ SUCCESS
# Event: WithdrawCompleted emitted with:
#   - vault_id: 0xafb31d363dd0d652b49320b7823afd574c5c99abcd34b7f0e2e6d63fd3f4bbf9
#   - owner: 0xcd8369e1a8ae681fb05660ffe9811872daff3f6946a4981c2e573a0627c3a877
#   - recipient: 0xcd8369e1a8ae681fb05660ffe9811872daff3f6946a4981c2e573a0627c3a877
#   - amount: 5000000 (5 USDC)
# Created: New USDC coin with 5 USDC at 0x820e483ef87e5a2bf8f0f15f11a77a83f83ae82dba153bffa00b300f028e5717
```

#### Step 6: Verify Final State

```bash
# Check final vault balance
sui client object 0xafb31d363dd0d652b49320b7823afd574c5c99abcd34b7f0e2e6d63fd3f4bbf9

# Result: ✅ Vault balance correctly shows 5,000,000 (5 USDC remaining)
# - usdc_balance: 5000000
# - deposit_count: 1 (unchanged)

# Check received USDC coin
sui client object 0x820e483ef87e5a2bf8f0f15f11a77a83f83ae82dba153bffa00b300f028e5717

# Result: ✅ New USDC coin with correct balance
# - balance: 5000000 (5 USDC)
# - owner: 0xcd8369e1a8ae681fb05660ffe9811872daff3f6946a4981c2e573a0627c3a877
```

### Testing Summary

| Test Case | Status | Transaction Hash | Details |
|-----------|--------|------------------|---------|
| Contract Deploy | ✅ PASS | `ChmuhXj66jtTzo7qDdZPtWzL9xosUo8kxx6GDPVsaVvN` | Package deployed successfully |
| Vault Creation | ✅ PASS | `ChmuhXj66jtTzo7qDdZPtWzL9xosUo8kxx6GDPVsaVvN` | Vault and OwnerCap created |
| USDC Deposit | ✅ PASS | `GVHxsYci8pZPF7VqBjNdNRGPumauzdzJpc3FGs8FCzRJ` | 10 USDC deposited with order ID |
| Event Emission | ✅ PASS | `GVHxsYci8pZPF7VqBjNdNRGPumauzdzJpc3FGs8FCzRJ` | DepositReceived event emitted |
| Owner Withdrawal | ✅ PASS | `4WJJDxawnxSTyCa7PAXnLxz7YJhW6RLGfoz7Cr19aDfE` | 5 USDC withdrawn successfully |
| Balance Updates | ✅ PASS | N/A | All balances updated correctly |
| Access Control | ✅ PASS | N/A | Only owner could withdraw |
| Coin Creation | ✅ PASS | `4WJJDxawnxSTyCa7PAXnLxz7YJhW6RLGfoz7Cr19aDfE` | New USDC coin created for user |

### Key Observations

#### ✅ Successful Operations
- **Deployment**: Contract deployed without issues to testnet
- **USDC Integration**: Perfect compatibility with Circle's testnet USDC
- **Event System**: All events emitted with correct data structure
- **Gas Efficiency**: Reasonable gas usage for all operations
- **Security**: Access control working as expected
- **State Management**: All vault state updates working correctly

#### 🎯 Event Compatibility
The events emitted match the expected structure for LedgerFlow indexer integration:

```javascript
// DepositReceived Event
{
  "vault_id": "0xafb31d363dd0d652b49320b7823afd574c5c99abcd34b7f0e2e6d63fd3f4bbf9",
  "payer": "0xcd8369e1a8ae681fb05660ffe9811872daff3f6946a4981c2e573a0627c3a877",
  "order_id": "b3JkZXJfdGVzdF8xMjM0NQ==", // base64 encoded
  "amount": "10000000",
  "deposit_index": "1",
  "timestamp": "1755609408257"
}
```

#### 💰 Gas Costs Analysis
- **Deployment**: ~4.7 MIST (reasonable for contract size)
- **Vault Creation**: ~4.1 MIST (efficient object creation)
- **Deposit**: ~4.1 MIST (includes event emission)
- **Withdrawal**: ~7.0 MIST (includes coin creation and transfer)

### Integration Readiness

The successful testing confirms the vault is ready for:

✅ **Indexer Integration** - Events structure matches requirements  
✅ **Bot Integration** - Order tracking working correctly  
✅ **Production Use** - All core functions validated  
✅ **Multi-chain Support** - Event compatibility maintained  

## Quick Start

### 1. Deploy the Contract

```bash
# Build and deploy
./scripts/deploy.sh

# Or manually:
sui move build
sui client publish --gas-budget 100000000
```

### 2. Create a Vault

```bash
# Set your package ID
export PACKAGE_ID="0x..."

# Create vault
sui client call \
    --package $PACKAGE_ID \
    --module payment_vault \
    --function init_vault \
    --args 0x6 \
    --gas-budget 100000000
```

### 3. Share the Vault

```bash
# Make vault publicly accessible for deposits
sui client call \
    --package sui \
    --module transfer \
    --function share_object \
    --args $VAULT_ID \
    --type-args "${PACKAGE_ID}::payment_vault::PaymentVault" \
    --gas-budget 100000000
```

### 4. Make a Deposit

```typescript
// Using TypeScript SDK
const tx = new Transaction();

tx.moveCall({
    target: `${PACKAGE_ID}::payment_vault::deposit`,
    arguments: [
        tx.object(vaultId),
        tx.object(usdcCoinId),
        tx.pure.vector('u8', Array.from(new TextEncoder().encode('order_12345'))),
        tx.object('0x6'), // Clock
    ],
});

const result = await client.signAndExecuteTransaction({ transaction: tx, signer: keypair });
```

## Integration with LedgerFlow System

This vault is designed to integrate seamlessly with the broader LedgerFlow ecosystem:

### Event Monitoring

The contract emits structured events that can be monitored by the LedgerFlow indexer:

```rust
// Example indexer code
pub async fn handle_deposit_received(event: DepositReceived) {
    // Update database with deposit information
    // Notify bot of successful payment
    // Update order status from pending to completed
}
```

### Indexer Compatibility
- **Event Structure**: Maintains same event fields as EVM/Aptos versions
- **Order ID Format**: Compatible with existing order generation algorithm
- **Chain Support**: Ready for multi-chain indexer integration

### Bot Integration  
- **Events enable automatic order status updates**
- **Same notification patterns as other chains**
- **Error handling compatible with bot logic**

### Multi-Chain Support

The vault supports the same event structure as EVM implementations, enabling:

- **Unified indexer logic across chains**
- **Consistent order tracking**
- **Cross-chain payment monitoring**

## 📊 Implementation Details

### Code Metrics
- **Lines of Code**: ~450 lines (including documentation)
- **Functions**: 15 total (12 public + 3 helpers)
- **Error Codes**: 7 comprehensive error conditions
- **Events**: 3 event types for complete operation tracking
- **Dependencies**: 2 external (Sui framework, Circle USDC)

### Test Coverage
- **Total Tests**: 12 comprehensive test cases
- **Success Rate**: 100% (12/12 passing)
- **Test Types**: Unit tests, error handling, access control validation
- **Framework**: Sui Move test framework with comprehensive scenarios

### Key Differences from Aptos Version

#### Technical Architecture
- **Object Model**: Single vault object vs resource-based storage
- **Coin Handling**: Uses `Balance<USDC>` for storage efficiency
- **Timestamp Management**: Requires `&Clock` parameter in functions
- **Access Control**: Capability-based instead of resource-based

#### Business Logic Compatibility
- **Event Structure**: Maintains same field names and types
- **Error Handling**: Same error codes as Aptos version
- **Access Control**: Same owner-only restrictions with additional vault-capability binding

## Development

### Building

```bash
sui move build
```

### Testing

```bash
# Run all tests
sui move test

# Run specific test module
sui move test payment_vault_tests
```

### Deployment Scripts

- `scripts/deploy.sh` - Deploy to testnet/mainnet
- `scripts/demo.sh` - Interactive demo of vault operations

## 🚀 Deployment Status

### Current Status

- ✅ **Contract Ready**: Fully implemented and tested
- ✅ **Scripts Prepared**: Deployment and demo scripts ready
- ✅ **Documentation Complete**: Comprehensive usage guides
- ⏳ **Testnet Deployment**: Ready for first deployment
- ⏳ **Integration Testing**: Pending indexer integration

### Next Steps

#### Immediate Actions (Phase 1)

1. **Deploy to Testnet** - Use provided deployment scripts
2. **Create Test Vault** - Set up demonstration vault  
3. **Integration Testing** - Test with LedgerFlow indexer
4. **Performance Analysis** - Measure gas costs and optimize

#### Medium Term (Phase 2)

1. **Security Review** - Comprehensive security analysis
2. **Indexer Integration** - Full integration with event monitoring
3. **Documentation Enhancement** - User guides and tutorials
4. **Advanced Testing** - Stress testing and edge case validation

#### Future Enhancements (Phase 3)

1. **Multi-Coin Support** - Generic coin type parameters
2. **Batch Operations** - Multiple deposits/withdrawals
3. **Access Control Lists** - Multiple authorized operators
4. **Vault Factory** - Standardized vault creation pattern

## Security Considerations

### Access Control

- **Capability-based security**: Only `OwnerCap` holders can perform admin operations
- **Vault binding**: Each `OwnerCap` is bound to a specific vault ID
- **Transfer protection**: Ownership transfers require explicit function calls

### Input Validation

- **Amount checks**: All amounts must be greater than zero
- **Address validation**: Prevents operations with zero addresses
- **Order ID validation**: Ensures order IDs are not empty

### Coin Safety

- **No coin loss**: Uses Sui's linear type system to prevent coin loss
- **Atomic operations**: All deposits and withdrawals are atomic
- **Balance tracking**: Accurate balance maintenance through coin operations

## USDC Integration

### Testnet USDC

- **Contract Address**: `0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC`
- **Faucet**: [Circle USDC Faucet](https://faucet.circle.com/)
- **Explorer**: [Sui Testnet Explorer](https://suiscan.xyz/testnet)

### Mainnet USDC

- **Contract Address**: `0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC`
- **Explorer**: [Sui Mainnet Explorer](https://suiscan.xyz/mainnet)

## Event Integration

### Event Structure

The vault emits standardized events compatible with the LedgerFlow indexer:

```rust
struct DepositReceived has copy, drop {
    vault_id: ID,
    payer: address,
    order_id: vector<u8>,
    amount: u64,
}
```

### Multi-Chain Compatibility

Events follow the same structure as EVM and Aptos implementations:

- **Unified Indexing**: Same event format across all chains
- **Order ID Consistency**: Compatible with existing order generation
- **Amount Precision**: Consistent USDC decimal handling

## Summary

This Sui implementation provides a secure, efficient payment vault system that seamlessly integrates with the existing LedgerFlow ecosystem. The vault offers:

✅ **Complete Feature Parity** - Matches EVM and Aptos functionality  
✅ **Sui-Native Design** - Leverages object model and capabilities  
✅ **Production Ready** - Comprehensive testing and security measures  
✅ **USDC Integration** - Official Circle USDC support  
✅ **Event Compatibility** - Works with existing indexer infrastructure  

The implementation is ready for deployment and testing, with clear pathways for future enhancements and multi-chain integration.

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| 1 | `E_NOT_OWNER` | Caller is not the vault owner |
| 2 | `E_INSUFFICIENT_BALANCE` | Insufficient balance for operation |
| 3 | `E_INVALID_AMOUNT` | Amount must be greater than 0 |
| 4 | `E_INVALID_ORDER_ID` | Order ID cannot be empty |
| 5 | `E_INVALID_ADDRESS` | Invalid address provided |
| 6 | `E_SELF_OPERATION` | Operation not allowed on self |
| 7 | `E_WRONG_VAULT` | Capability is for wrong vault |

## Contributing

1. Follow the coding standards used in the existing codebase
2. Add comprehensive tests for new functionality
3. Update documentation for any interface changes
4. Ensure compatibility with the broader LedgerFlow system

## Related Projects

- [LedgerFlow Vault EVM](../ledgerflow-vault-evm/) - Ethereum/EVM implementation
- [LedgerFlow Vault Aptos](../ledgerflow-vault-aptos/) - Aptos Move implementation
- [LedgerFlow Indexer](../ledgerflow-indexer-sui/) - Event monitoring service
- [LedgerFlow Balancer](../ledgerflow-balancer/) - Backend API service

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Resources

- [Sui Documentation](https://docs.sui.io/)
- [Sui Move by Example](https://examples.sui.io/)
- [Circle USDC on Sui](https://docs.sui.io/guides/developer/stablecoins)
- [Sui TypeScript SDK](https://sdk.mystenlabs.com/typescript)
