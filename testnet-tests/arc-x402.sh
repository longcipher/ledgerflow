#!/usr/bin/env bash
# Real x402 (EIP-3009) payment test on Arc Testnet (chainId 5042002).
#
# The x402 `exact` scheme on EVM settles via EIP-3009 TransferWithAuthorization:
#   1. Client (agent) signs an EIP-712 TransferWithAuthorization typed data.
#   2. The merchant/facilitator submits it via USDC.transferWithAuthorization
#      and USDC moves from payer to payee.
#
# This test drives the FULL x402 semantics:
#   - 402 PaymentRequired challenge (merchant advertises USDC requirement)
#   - client-side EIP-712 signature (via onecipher)
#   - on-chain settlement (via cast -> transferWithAuthorization)
#
# Requirements:
#   - cast (foundry) in PATH
#   - onecipher binary in PATH or ONECIPHER_BIN
#   - Wallet imported as `arc-test` in onecipher (EVM, given test key)

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
RPC="${ARC_RPC:-https://rpc.testnet.arc.io}"
USDC="0x3600000000000000000000000000000000000000"
PAYER="0x00B8E3e3d589577bAeAbbE0f72993e5F72e82f00"
WALLET="${ONECIPHER_WALLET:-arc-test}"
ONECIPHER_BIN="${ONECIPHER_BIN:-$(command -v onecipher || echo /home/akagi201/src/github.com/longcipher/onecipher/target/debug/onecipher)}"

# Merchant payee (generated once, persist between runs in merchant.txt)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MERCHANT_FILE="$SCRIPT_DIR/merchant.txt"
if [[ -f "$MERCHANT_FILE" ]]; then
    MERCHANT_PK="$(grep PK "$MERCHANT_FILE" | cut -d= -f2)"
    PAYEE="$(grep ADDR "$MERCHANT_FILE" | cut -d= -f2)"
else
    CAST_WALLET_OUT="$(cast wallet new)"
    MERCHANT_PK="$(echo "$CAST_WALLET_OUT" | grep -o '0x[0-9a-fA-F]\{64\}' | head -1)"
    PAYEE="$(cast wallet address "$MERCHANT_PK" | head -1)"
    printf 'PK=%s\nADDR=%s\n' "$MERCHANT_PK" "$PAYEE" > "$MERCHANT_FILE"
fi

# Amount: 0.25 USDC (6 decimals)
AMOUNT=250000
AMOUNT_HUMAN="0.25"

NOW="$(date +%s)"
VALID_AFTER="$((NOW - 60))"          # valid immediately
VALID_BEFORE="$((NOW + 3600))"       # valid for 1 hour
NONCE="0x$(printf 'x402-arc-%s' "$NOW" | sha256sum | cut -c1-64)"  # 32-byte nonce
CHAIN_ID=5042002

log() { echo "[x402-arc] $*"; }

# ---------------------------------------------------------------------------
# Step 0: Pre-check balances
# ---------------------------------------------------------------------------
log "=== Step 0: Pre-check ==="
log "payer=$PAYER  payee=$PAYEE"
log "amount=${AMOUNT_HUMAN} USDC (${AMOUNT} base units)"
PAYER_BAL_BEFORE="$(cast call --rpc-url "$RPC" "$USDC" "balanceOf(address)(uint256)" "$PAYER" | tr -d '\n')"
PAYEE_BAL_BEFORE="$(cast call --rpc-url "$RPC" "$USDC" "balanceOf(address)(uint256)" "$PAYEE" | tr -d '\n')"
log "payer balance before: $PAYER_BAL_BEFORE"
log "payee balance before: $PAYEE_BAL_BEFORE"

# ---------------------------------------------------------------------------
# Step 1: Merchant issues 402 PaymentRequired (x402 challenge)
# ---------------------------------------------------------------------------
log ""
log "=== Step 1: Merchant 402 PaymentRequired (x402 challenge) ==="
log "Merchant advertises: scheme=exact, asset=USDC@arc:5042002, payTo=$PAYEE, amount=$AMOUNT"
cat <<EOF
HTTP/1.1 402 Payment Required
content-type: application/json
x-payment-required: base64(x402 v2 PaymentRequired)

{
  "x402Version": 2,
  "error": "Payment required",
  "resource": { "url": "https://merchant.example/api/data", "description": "Premium data" },
  "accepts": [{
    "scheme": "exact",
    "network": "eip155:5042002",
    "amount": "$AMOUNT",
    "asset": "$USDC",
    "payTo": "$PAYEE",
    "maxTimeoutSeconds": 3600
  }],
  "extensions": { "ledgerflow": { "info": { "warrantRequired": false } } }
}
EOF

# ---------------------------------------------------------------------------
# Step 2: Client builds EIP-712 TransferWithAuthorization typed data
# ---------------------------------------------------------------------------
log ""
log "=== Step 2: Client EIP-712 signature (via onecipher) ==="
# EIP-3009 TransferWithAuthorization domain (FiatToken-style; verify against
# DOMAIN_SEPARATOR() on-chain):
DOMAIN_NAME="USD Coin"
DOMAIN_VERSION="2"
VERIFYING_CONTRACT="$USDC"
SALT="0x0000000000000000000000000000000000000000000000000000000000000000"

TYPED_DATA_JSON=$(cat <<JSON
{
  "types": {
    "EIP712Domain": [
      {"name": "name", "type": "string"},
      {"name": "version", "type": "string"},
      {"name": "chainId", "type": "uint256"},
      {"name": "verifyingContract", "type": "address"},
      {"name": "salt", "type": "bytes32"}
    ],
    "TransferWithAuthorization": [
      {"name": "from", "type": "address"},
      {"name": "to", "type": "address"},
      {"name": "value", "type": "uint256"},
      {"name": "validAfter", "type": "uint256"},
      {"name": "validBefore", "type": "uint256"},
      {"name": "nonce", "type": "bytes32"}
    ]
  },
  "primaryType": "TransferWithAuthorization",
  "domain": {
    "name": "$DOMAIN_NAME",
    "version": "$DOMAIN_VERSION",
    "chainId": "$CHAIN_ID",
    "verifyingContract": "$VERIFYING_CONTRACT",
    "salt": "$SALT"
  },
  "message": {
    "from": "$PAYER",
    "to": "$PAYEE",
    "value": "$AMOUNT",
    "validAfter": "$VALID_AFTER",
    "validBefore": "$VALID_BEFORE",
    "nonce": "$NONCE"
  }
}
JSON
)

log "typed data: $(echo "$TYPED_DATA_JSON" | tr -d '\n')"

SIG_RAW="$("$ONECIPHER_BIN" sign message --chain ethereum --wallet "$WALLET" --typed-data "$TYPED_DATA_JSON" 2>/dev/null | tr -d '\n')"
log "signature (r||s||v): $SIG_RAW"

# EIP-3009 signature layout: r (32) | s (32) | v (1 byte, 27/28)
R="0x$(echo "$SIG_RAW" | cut -c1-64)"
S="0x$(echo "$SIG_RAW" | cut -c65-128)"
V_HEX="$(echo "$SIG_RAW" | cut -c129-130)"
# onecipher returns v as 27 or 28 in last byte; normalize to 0/1 for cast
if [[ "$V_HEX" == "1b" ]]; then V=0; elif [[ "$V_HEX" == "1c" ]]; then V=1; else V=$((16#$V_HEX - 27)); fi
log "r=$R s=$S v=$V"

# ---------------------------------------------------------------------------
# Step 3: Merchant submits transferWithAuthorization (real settlement)
# ---------------------------------------------------------------------------
log ""
log "=== Step 3: On-chain settlement via USDC.transferWithAuthorization ==="
TX_HASH="$(cast send --rpc-url "$RPC" --private-key "$MERCHANT_PK" "$USDC" \
  "transferWithAuthorization(address,address,uint256,uint256,uint256,bytes32,uint8,bytes32,bytes32)" \
  "$PAYER" "$PAYEE" "$AMOUNT" "$VALID_AFTER" "$VALID_BEFORE" "$NONCE" "$V" "$R" "$S" 2>&1 | grep -o '0x[0-9a-f]\{64\}' | head -1)"
log "settlement tx: $TX_HASH"

# ---------------------------------------------------------------------------
# Step 4: Verify
# ---------------------------------------------------------------------------
log ""
log "=== Step 4: Verify settlement ==="
sleep 2
RECEIPT="$(cast receipt --rpc-url "$RPC" "$TX_HASH" 2>&1 | head -5)"
log "receipt:"
log "$RECEIPT"

PAYER_BAL_AFTER="$(cast call --rpc-url "$RPC" "$USDC" "balanceOf(address)(uint256)" "$PAYER" | tr -d '\n')"
PAYEE_BAL_AFTER="$(cast call --rpc-url "$RPC" "$USDC" "balanceOf(address)(uint256)" "$PAYEE" | tr -d '\n')"
log "payer balance after:  $PAYER_BAL_AFTER"
log "payee balance after:  $PAYEE_BAL_AFTER"

DELTA_PAYER=$((PAYER_BAL_BEFORE - PAYER_BAL_AFTER))
DELTA_PAYEE=$((PAYEE_BAL_AFTER - PAYEE_BAL_BEFORE))
log "payer delta:  -${DELTA_PAYER} base units"
log "payee delta:  +${DELTA_PAYEE} base units"

if [[ "$DELTA_PAYEE" == "$AMOUNT" && "$DELTA_PAYER" == "$AMOUNT" ]]; then
    log "✅ PASS: x402 exact settlement completed ($AMOUNT_HUMAN USDC moved payer->payee)"
    echo "TX=$TX_HASH" >> "$MERCHANT_FILE"
    exit 0
else
    log "❌ FAIL: settlement mismatch (expected ±$AMOUNT)"
    exit 1
fi
