# Scheme: `exact` on `Sui`

## Summary

The `exact` scheme on Sui chain uses `IntentScope::PersonalMessage` to authorize a transfer of a specific amount of a coin type from the payer to the resource server. This approach leverages Sui's intent signing mechanism to create structured authorization messages that can be submitted by third parties in future transactions, enabling "gasless" transactions for the Sui ecosystem.

Unlike EVM's `EIP-3009` which directly executes token transfers, Sui's implementation separates the authorization (intent signing) from execution (transaction submission), providing a more flexible foundation for gasless transaction primitives.

## `X-Payment` header payload

The `payload` field of the `X-PAYMENT` header must contain the following fields:

- `signature`: The base64-encoded signature of the structured personal message using Sui's intent signing format.
- `authorization`: Parameters required to reconstruct the signed message and validate the payment authorization.
- `gasBudget`: Optional gas budget for transaction execution (Sui extension). Unlike EVM where gas is implicitly handled, Sui requires explicit gas budget specification for transaction execution control.

Example:

```json
{
  "signature": "AQNqaXN0aGlzaXNhbW9ja3NpZ25hdHVyZWZvcnRlc3RpbmdwdXJwb3Nlcy4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4u",
  "authorization": {
    "from": "0xa11cea3bbf6889be9d49991757cb0b676d644e24acec54225d64c97a1d81acc1",
    "to": "0xb0be0b86d3fa8ad7484d88821c78c035fa819702a0cc06cf2a4fc4924036a885",
    "value": "1000000",
    "validAfter": "1740672089",
    "validBefore": "1740672154",
    "nonce": "0xf3746613c2d920b5fdabc0856f2aeb2d4f88ee6037b8cc5d04a71a4462f13480",
    "coinType": "0x2::sui::SUI"
  },
  "gasBudget": 10000000
}
```

Full `X-PAYMENT` header:

```json
{
  "x402Version": 1,
  "scheme": "exact",
  "network": "sui-testnet",
  "payload": {
    "signature": "AQNqaXN0aGlzaXNhbW9ja3NpZ25hdHVyZWZvcnRlc3RpbmdwdXJwb3Nlcy4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4uLi4u",
    "authorization": {
      "from": "0xa11cea3bbf6889be9d49991757cb0b676d644e24acec54225d64c97a1d81acc1",
      "to": "0xb0be0b86d3fa8ad7484d88821c78c035fa819702a0cc06cf2a4fc4924036a885",
      "value": "1000000",
      "validAfter": "1740672089",
      "validBefore": "1740672154",
      "nonce": "0xf3746613c2d920b5fdabc0856f2aeb2d4f88ee6037b8cc5d04a71a4462f13480",
      "coinType": "0x2::sui::SUI"
    },
    "gasBudget": 10000000
  }
}
```

## Structured Personal Message Format

The signed message uses `IntentScope::PersonalMessage` with a standardized JSON structure:

```json
{
  "intent": {
    "scope": "PersonalMessage",
    "version": "V0",
    "appId": "Sui"
  },
  "authorization": {
    "from": "0xa11cea3bbf6889be9d49991757cb0b676d644e24acec54225d64c97a1d81acc1",
    "to": "0xb0be0b86d3fa8ad7484d88821c78c035fa819702a0cc06cf2a4fc4924036a885",
    "value": "1000000",
    "validAfter": 1740672089,
    "validBefore": 1740672154,
    "nonce": "0xf3746613c2d920b5fdabc0856f2aeb2d4f88ee6037b8cc5d04a71a4462f13480",
    "coinType": "0x2::sui::SUI"
  }
}
```

### Message Signing Process

1. **Construct the structured message** following the format above
2. **Serialize to JSON** with consistent formatting (no extra whitespace)
3. **Convert to bytes** using UTF-8 encoding
4. **Sign with IntentScope::PersonalMessage** using user's private key
5. **Encode signature** in base64 format for transmission

### Authorization Fields

- `from`: Sui address of the payer
- `to`: Sui address of the recipient (payment processor)
- `value`: Amount in base units (e.g., MIST for SUI, micro-units for USDC)
- `validAfter`: Unix timestamp after which the authorization becomes valid
- `validBefore`: Unix timestamp after which the authorization expires
- `nonce`: 32-byte hex-encoded unique identifier to prevent replay attacks
- `coinType`: Full Sui coin type identifier (e.g., `0x2::sui::SUI`)

## Verification

Steps to verify a payment for the `exact` scheme on Sui:

1. **Verify signature validity**
   - Reconstruct the structured personal message
   - Verify the signature against the message using the `from` address's public key
   - Ensure the signature uses `IntentScope::PersonalMessage`

2. **Verify payer balance**
   - Query the payer's balance for the specified `coinType`
   - Ensure balance covers `paymentRequirements.maxAmountRequired`

3. **Verify authorization amount**
   - Ensure `payload.authorization.value` meets `paymentRequirements.maxAmountRequired`

4. **Verify timing constraints**
   - Check current timestamp is after `validAfter`
   - Check current timestamp is before `validBefore`

5. **Verify nonce uniqueness**
   - Ensure the nonce hasn't been used in previous transactions
   - Store nonce to prevent replay attacks

6. **Verify coin type compatibility**
   - Ensure `coinType` matches the expected asset type
   - Verify the coin type exists and is active on the network

7. **Verify network compatibility**
   - Ensure the authorization is for the correct Sui network (mainnet/testnet/devnet)

8. **Simulate transaction execution**
   - Dry-run the payment transaction to ensure it would succeed
   - Verify gas budget is sufficient for execution

## Settlement

Settlement on Sui is performed by the facilitator constructing and submitting a transaction that:

1. **Splits coins** from the payer's account of the specified `coinType`
2. **Transfers the exact amount** to the recipient address
3. **Pays gas fees** using the facilitator's account (enabling gasless transactions)
4. **Emits events** for payment tracking and reconciliation

### Settlement Transaction Structure

```rust
// Pseudo-code for settlement transaction
let tx = TransactionBuilder::new()
    .split_coins(
        payer_address,
        coin_type,
        authorization.value
    )
    .transfer_to_recipient(
        recipient_address,
        authorization.value
    )
    .with_gas_payment(facilitator_address)
    .build();
```

### LedgerFlow PaymentVault Integration

For LedgerFlow's specific implementation, settlement integrates with the PaymentVault contract:

```rust
// Settlement via PaymentVault
public entry fun deposit_with_authorization(
    vault: &mut PaymentVault,
    payment_coin: Coin<T>,
    order_id: vector<u8>,
    ctx: &mut TxContext
) {
    // Validate authorization (off-chain verification)
    // Deposit coin to vault
    // Emit DepositReceived event with order_id mapping
}
```

## Gasless Transaction Primitives

This scheme provides the foundation for gasless transactions in the Sui ecosystem:

### User Experience Flow

1. **User signs intent** without submitting transaction
2. **Application receives signed authorization**
3. **Third party (facilitator) submits transaction** and pays gas
4. **User's payment is processed** without requiring SUI for gas

### Security Model

- **Intent separation**: Authorization is separate from execution
- **Time-bounded validity**: Prevents indefinite authorization abuse
- **Nonce protection**: Prevents replay attacks
- **Amount precision**: Exact amount authorization prevents overpayment

### Integration Benefits

- **Wallet simplification**: Users don't need SUI for gas
- **Better UX**: Seamless payment flow
- **Facilitator flexibility**: Third parties can optimize gas strategies
- **Ecosystem growth**: Lower barrier to entry for new users

## Sui-Specific Considerations

### Gas Budget Management

Unlike EVM chains where gas is implicitly handled, Sui requires explicit `gasBudget` specification for several critical reasons:

#### Why gasBudget is Required

1. **Transaction Execution Control**

   ```rust
   // Sui transaction MUST specify gas budget
   let tx = TransactionBuilder::new()
       .move_call(target, function, args)
       .with_gas_budget(10_000_000) // Required
       .build();
   ```

2. **Facilitator Cost Protection**
   - Prevents unlimited gas consumption by malicious users
   - Allows facilitators to set maximum gas cost they're willing to pay
   - Enables predictable cost modeling for gasless transaction services

3. **Transaction Reliability**
   - Pre-execution validation ensures sufficient gas budget
   - Prevents transaction failures due to insufficient gas
   - Provides clear error messages when budget is inadequate

#### Gas Budget Best Practices

- **Conservative estimates**: Set gasBudget 20-30% higher than expected consumption
- **Network-specific values**: Different networks may have different gas costs
- **Operation complexity**: More complex transactions require higher budgets
- **Failure handling**: Implement fallback mechanisms for gas budget failures

#### Example Gas Budget Values

```json
{
  "gasBudget": 10000000,  // Standard payment: ~10M gas units
  "gasBudget": 50000000,  // Complex operations: ~50M gas units
  "gasBudget": 100000000  // Batch operations: ~100M gas units
}
```

### Coin Type System

Sui's flexible coin type system allows for:

- **Native SUI**: `0x2::sui::SUI`
- **Wrapped tokens**: Custom coin types with full type paths
- **Dynamic coin types**: Support for new assets without protocol changes

### Object Model Integration

Sui's object-centric model enables:

- **Shared objects**: PaymentVault as shared object for global access
- **Owned objects**: User coin objects for payment source
- **Dynamic fields**: Extensible metadata for payment tracking

### Move Language Benefits

- **Type safety**: Compile-time guarantees for coin types
- **Resource semantics**: Prevents double-spending at language level
- **Capability patterns**: Secure delegation of payment authorization

## Appendix

### Comparison with EVM Implementation

| Aspect | EVM (EIP-3009) | Sui (PersonalMessage) |
|--------|----------------|----------------------|
| **Authorization** | EIP-3009 signature | IntentScope::PersonalMessage |
| **Execution** | Direct contract call | Facilitator transaction |
| **Gas Payment** | Facilitator pays | Facilitator pays |
| **Gas Budget** | Implicit/automatic | Explicit gasBudget required |
| **Replay Protection** | Nonce in contract | Nonce tracking |
| **Coin Types** | Single ERC20 | Flexible coin types |
| **Settlement** | transferWithAuthorization | Custom transaction |

### Future Enhancements

1. **Batch payments**: Multiple authorizations in single transaction
2. **Conditional payments**: Smart contract-based conditions
3. **Recurring payments**: Time-based authorization renewal
4. **Cross-chain support**: Bridge integration for multi-chain payments

### Security Considerations

- **Key management**: Secure storage of signing keys
- **Nonce synchronization**: Prevent nonce conflicts in distributed systems
- **Time synchronization**: Ensure consistent timestamp validation
- **Amount validation**: Prevent precision loss in amount conversions
- **Gas budget limits**: Set reasonable upper bounds to prevent DoS attacks via excessive gas consumption

### Recommendations

- Use `IntentScope::PersonalMessage` for all payment authorizations
- Implement robust nonce tracking to prevent replay attacks
- Support flexible coin types for ecosystem growth
- Maintain compatibility with LedgerFlow's order tracking system
- Consider gas optimization strategies for high-volume settlements

This scheme provides a robust foundation for exact payment authorization on Sui while enabling innovative gasless transaction patterns that benefit the entire ecosystem.
