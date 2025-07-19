# LedgerFlow Payment Vault Interaction Scripts

This directory contains scripts for interacting with the LedgerFlow Payment Vault deployed on Aptos testnet using the Aptos CLI.

## Contract Information

- **Contract Address**: `0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846`
- **Network**: Aptos Testnet
- **USDC Metadata**: `0x69091fbab5f7d635ee7ac5098cf0c1efbe31d68fec0f2cd565e8d168daf52832`
- **Explorer**: [View on Aptos Explorer](https://explorer.aptoslabs.com/account/0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846?network=testnet)

## Prerequisites

1. **Install Aptos CLI**

   ```bash
   curl -fsSL "https://aptos.dev/scripts/install_cli.py" | python3
   ```

2. **Initialize Aptos CLI Profile**

   ```bash
   aptos init --network testnet
   ```

3. **Get Testnet USDC**

   - Visit [Circle Testnet Faucet](https://faucet.circle.com/)
   - Request testnet USDC for your account

4. **Install Required Tools**

   ```bash
   # For generating random order IDs
   brew install openssl  # macOS
   # or
   sudo apt-get install openssl  # Ubuntu

   # For decimal calculations
   brew install bc  # macOS
   # or
   sudo apt-get install bc  # Ubuntu
   ```

## Scripts Overview

### 1. `vault_test.sh` - Comprehensive Test Script

This is the main script that provides multiple functionalities:

```bash
# Check vault status and balances
./scripts/vault_test.sh status

# Deposit 1 USDC
./scripts/vault_test.sh deposit

# Deposit custom amount (in micro-USDC)
./scripts/vault_test.sh deposit 5000000  # 5 USDC

# Withdraw all funds to current account
./scripts/vault_test.sh withdraw

# Withdraw all funds to specific address
./scripts/vault_test.sh withdraw 0x123...

# Run full test (deposit then withdraw)
./scripts/vault_test.sh full-test
```

### 2. `deposit_1_usdc.sh` - Simple Deposit Script

Deposits exactly 1 USDC to the vault:

```bash
./scripts/deposit_1_usdc.sh
```

Features:

- Generates random order ID
- Validates USDC balance
- Executes deposit transaction
- Provides transaction links

### 3. `withdraw_all_usdc.sh` - Withdrawal Script

Withdraws all USDC from the vault (owner only):

```bash
./scripts/withdraw_all_usdc.sh
```

Features:
- Verifies vault ownership
- Checks current balance
- Prompts for recipient address
- Requires confirmation before withdrawal

## Move Scripts

The repository also includes Move script files for more advanced usage:

- `deposit_usdc.move` - Move script for deposits
- `withdraw_all.move` - Move script for withdrawals
- `deploy_fa.move` - Deployment script

## Usage Examples

### Basic Deposit and Withdrawal

1. **Check vault status**:
   ```bash
   ./scripts/vault_test.sh status
   ```

2. **Deposit 1 USDC**:
   ```bash
   ./scripts/deposit_1_usdc.sh
   ```

3. **Withdraw all funds**:
   ```bash
   ./scripts/withdraw_all_usdc.sh
   ```

### Using the Test Script

1. **Run comprehensive test**:
   ```bash
   ./scripts/vault_test.sh full-test
   ```

2. **Deposit specific amount**:
   ```bash
   # Deposit 10 USDC (10 * 1,000,000 micro-USDC)
   ./scripts/vault_test.sh deposit 10000000
   ```

3. **Withdraw to specific address**:
   ```bash
   ./scripts/vault_test.sh withdraw 0xrecipient_address
   ```

## Transaction Flow

### Deposit Transaction
1. Script generates a unique order ID (32-byte hex string)
2. Calls `payment_vault_fa::deposit` function
3. Transfers USDC from user's primary store to vault
4. Emits `DepositReceived` event
5. Updates vault deposit counter

### Withdrawal Transaction
1. Verifies caller is vault owner
2. Calls `payment_vault_fa::withdraw_all` function
3. Transfers all USDC from vault to recipient
4. Emits `WithdrawCompleted` event

## Error Handling

The scripts include comprehensive error handling for:
- Missing Aptos CLI profile
- Insufficient USDC balance
- Invalid vault address
- Permission errors (withdrawal by non-owner)
- Network connectivity issues

## Security Notes

1. **Order IDs**: Each deposit generates a cryptographically secure random order ID
2. **Owner Verification**: Withdrawals require vault owner's signature
3. **Amount Validation**: All amounts are validated for positive values
4. **Network Safety**: All transactions target testnet to prevent mainnet accidents

## Troubleshooting

### Common Issues

1. **"No Aptos CLI profile found"**
   ```bash
   aptos init --network testnet
   ```

2. **"No USDC balance found"**
   - Get testnet USDC from https://faucet.circle.com/
   - Ensure you're using the correct account

3. **"Not vault owner"**
   - Only the vault owner can withdraw funds
   - Check if you're using the correct account

4. **Transaction failures**
   - Check network connectivity
   - Verify sufficient gas fees
   - Ensure USDC balance is sufficient

### Viewing Transactions

All successful transactions can be viewed on the Aptos Explorer:
- Main account: https://explorer.aptoslabs.com/account/0xd2b5bb7d81b7fa4eeae1b5f6d6a8e1f9cdc738189a1dcc2315ba4bb846?network=testnet
- Your account: Replace with your address in the URL

## Development Notes

- USDC uses 6 decimal places (1 USDC = 1,000,000 micro-USDC)
- Order IDs are 32-byte hex strings for uniqueness
- All scripts target Aptos testnet by default
- Scripts use the Fungible Asset (FA) standard, not legacy Coin standard
