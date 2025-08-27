# OrderId Removal from x402 Sui Scheme

## Summary

Removed the `orderId` field from the Sui x402 payment scheme to simplify the protocol and eliminate redundancy with the existing `nonce` field.

## Rationale

### Why Remove orderId?

1. **Redundancy**: `nonce` already provides uniqueness guarantees
   - 32-byte hex-encoded values are virtually impossible to collide
   - Serves the same purpose as order tracking at the protocol level

2. **Protocol Purity**: x402 should focus on payment authorization
   - `orderId` is application-layer concept, not protocol-layer
   - Keeps the scheme aligned with EVM implementation philosophy

3. **Simplification**: Reduces payload size and complexity
   - Fewer fields to validate and manage
   - Cleaner API surface

### Alternative Approaches for Order Tracking

If order tracking is still needed at the application layer:

1. **Nonce-to-OrderId Mapping**: Maintain mapping in application database
2. **Nonce Generation**: Use deterministic nonce generation based on order_id
3. **Event Correlation**: Use transaction events for order correlation

## Changes Made

### 1. Protocol Definition (`docs/scheme_exact_sui.md`)
- Removed `orderId` from payload field descriptions
- Updated JSON examples to exclude `orderId`
- Maintained `gasBudget` as Sui-specific extension

### 2. Type Definitions (`ledgerflow-facilitator/src/types.rs`)
- Removed `order_id: Vec<u8>` from `SuiPayload` struct
- Preserved all other fields for compatibility

### 3. CLI Tools (`ledgerflow-sui-cli/src/main.rs`)
- Removed `orderId` from facilitator payload generation
- Simplified JSON structure in `send_to_facilitator` function

### 4. Test Files
Updated all test cases to remove `orderId`:
- `tests/facilitator_api_test.sh` (3 instances)
- `tests/facilitator_integration_test.sh` (4 instances)  
- `tests/cli_intent_test.sh` (1 instance)
- `test_payload.json` (1 instance)

## Breaking Change Impact

This is a **breaking change** that affects:

### API Compatibility
- Old clients sending `orderId` field will continue to work (field ignored)
- Protocol validation focuses on required fields only

### JSON Payload Size
- Reduced from ~658-661 characters to ~624 characters
- Saves approximately 34-37 bytes per request

### Migration Path
For applications that still need order tracking:

```rust
// Option 1: Database mapping
INSERT INTO order_nonce_mapping (nonce, order_id) VALUES (?, ?);

// Option 2: Deterministic nonce from order_id  
fn generate_nonce_from_order_id(order_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(order_id);
    hasher.finalize().into()
}

// Option 3: Extract from transaction events
// Use transaction hash and events for correlation
```

## Testing Status

### ✅ Completed
- [x] Protocol documentation updated
- [x] Type definitions modified  
- [x] CLI tools updated
- [x] All test files updated
- [x] Code compilation verified

### 🔄 In Progress
- [ ] Full integration testing with facilitator
- [ ] Event correlation testing  
- [ ] Performance impact measurement

## Conclusion

Removing `orderId` simplifies the x402 Sui scheme while maintaining all essential functionality. The `nonce` field provides sufficient uniqueness for protocol-level operations, and application-level order tracking can be implemented through various proven patterns.

This change aligns the Sui implementation more closely with x402 protocol principles and removes unnecessary complexity from the payment authorization process.
