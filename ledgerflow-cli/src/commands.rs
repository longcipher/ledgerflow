use alloy::{
    primitives::{FixedBytes, U256},
    providers::Provider,
    sol_types::SolEvent,
};
use eyre::{Result, eyre};

use crate::{
    contracts::{PaymentVault, USDC},
    lib_utils::{create_provider_with_wallet, format_usdc_amount, parse_address, parse_order_id},
};

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

    // Get wallet address
    let wallet_address = provider.get_accounts().await?[0];
    println!("Wallet address: {wallet_address}");

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
    let deposit_tx = vault_contract
        .deposit(order_id_bytes, amount_u256)
        .send()
        .await?;
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

/// Execute deposit operation with permit signature
pub async fn execute_deposit_with_permit(
    rpc_url: String,
    private_key: String,
    contract_address: String,
    order_id: String,
    amount: u64,
    deadline: u64,
) -> Result<()> {
    println!("=== Executing Permit Deposit Operation ===");

    // Parse parameters
    let contract_addr = parse_address(&contract_address)?;
    let order_id_bytes = parse_order_id(&order_id)?;
    let amount_u256 = U256::from(amount);

    // Create provider
    let provider = create_provider_with_wallet(&rpc_url, &private_key).await?;

    // Get wallet address
    let wallet_address = provider.get_accounts().await?[0];
    println!("Wallet address: {wallet_address}");

    // Create contract instance
    let vault_contract = PaymentVault::new(contract_addr, &provider);

    // Get USDC token address
    let usdc_address = vault_contract.usdcToken().call().await?;
    let usdc_contract = USDC::new(usdc_address, &provider);

    println!("PaymentVault contract address: {contract_addr}");
    println!("USDC token address: {usdc_address}");
    println!("Order ID: {order_id}");
    println!("Deposit amount: {} USDC", format_usdc_amount(amount_u256));
    println!("Permit deadline: {deadline}");

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

    // Get information required for permit
    let nonce = usdc_contract.nonces(wallet_address).call().await?;
    let domain_separator = usdc_contract.DOMAIN_SEPARATOR().call().await?;

    println!("Current nonce: {nonce}");
    println!("Domain separator: {domain_separator}");

    // Create permit signature
    // This is simplified, in production use EIP-712 standard signature
    // For demonstration, we use fixed v, r, s values
    // In production, implement EIP-712 signature properly
    let v = 27u8;
    let r = FixedBytes::from([0u8; 32]);
    let s = FixedBytes::from([0u8; 32]);

    println!(
        "⚠️  Note: This is a simplified example. In production, implement EIP-712 signature correctly."
    );

    // Execute deposit with permit
    println!("Executing permit deposit...");
    let deposit_tx = vault_contract
        .depositWithPermit(order_id_bytes, amount_u256, U256::from(deadline), v, r, s)
        .send()
        .await?;
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

    println!("✅ Permit deposit operation completed!");
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
    let wallet_address = provider.get_accounts().await?[0];
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
    let withdraw_tx = vault_contract.withdraw().send().await?;
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
