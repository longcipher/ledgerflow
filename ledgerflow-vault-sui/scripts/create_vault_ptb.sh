#!/bin/bash

PACKAGE_ID="0xd0a37165e44917ac53a2429d50b0edc26f8be103671e5a68167714a897f4d376"
USDC_TYPE="0xa1ec7fc00a6f40db9693ad1415d0c193ad3906494428cf252621037bd7117e29::usdc::USDC"
DEPLOYER="0xcd8369e1a8ae681fb05660ffe9811872daff3f6946a4981c2e573a0627c3a877"

echo "Creating vault with PTB..."

# Use PTB to properly handle multiple return values
sui client ptb \
  --move-call ${PACKAGE_ID}::payment_vault::init_vault\<${USDC_TYPE}\> @0x6 \
  --assign vault_and_cap \
  --split-coins vault_and_cap.0 vault_and_cap.1 \
  --transfer-objects '[vault_and_cap.1]' @${DEPLOYER} \
  --gas-budget 100000000
