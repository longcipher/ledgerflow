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

/// 执行标准存款操作
pub async fn execute_deposit(
    rpc_url: String,
    private_key: String,
    contract_address: String,
    order_id: String,
    amount: u64,
) -> Result<()> {
    println!("=== 执行存款操作 ===");

    // 解析参数
    let contract_addr = parse_address(&contract_address)?;
    let order_id_bytes = parse_order_id(&order_id)?;
    let amount_u256 = U256::from(amount);

    // 创建提供者
    let provider = create_provider_with_wallet(&rpc_url, &private_key).await?;

    // 获取钱包地址
    let wallet_address = provider.get_accounts().await?[0];
    println!("钱包地址: {}", wallet_address);

    // 创建合约实例
    let vault_contract = PaymentVault::new(contract_addr, &provider);

    // 获取USDC代币地址
    let usdc_address = vault_contract.usdcToken().call().await?;
    let usdc_contract = USDC::new(usdc_address, &provider);

    println!("PaymentVault 合约地址: {}", contract_addr);
    println!("USDC 代币地址: {}", usdc_address);
    println!("订单ID: {}", order_id);
    println!("存款金额: {} USDC", format_usdc_amount(amount_u256));

    // 检查USDC余额
    let balance = usdc_contract.balanceOf(wallet_address).call().await?;
    println!("当前USDC余额: {} USDC", format_usdc_amount(balance));

    if balance < amount_u256 {
        return Err(eyre!(
            "USDC余额不足。需要: {} USDC, 当前: {} USDC",
            format_usdc_amount(amount_u256),
            format_usdc_amount(balance)
        ));
    }

    // 检查USDC授权
    let allowance = usdc_contract
        .allowance(wallet_address, contract_addr)
        .call()
        .await?;
    println!("当前授权额度: {} USDC", format_usdc_amount(allowance));

    if allowance < amount_u256 {
        println!("授权额度不足，正在进行授权...");

        // 进行授权
        let approve_tx = usdc_contract
            .approve(contract_addr, amount_u256)
            .send()
            .await?;
        let approve_receipt = approve_tx.get_receipt().await?;

        println!("授权交易哈希: {}", approve_receipt.transaction_hash);
        println!(
            "授权交易状态: {}",
            if approve_receipt.status() {
                "成功"
            } else {
                "失败"
            }
        );

        if !approve_receipt.status() {
            return Err(eyre!("授权交易失败"));
        }
    }

    // 执行存款
    println!("正在执行存款...");
    let deposit_tx = vault_contract
        .deposit(order_id_bytes, amount_u256)
        .send()
        .await?;
    let deposit_receipt = deposit_tx.get_receipt().await?;

    println!("存款交易哈希: {}", deposit_receipt.transaction_hash);
    println!(
        "存款交易状态: {}",
        if deposit_receipt.status() {
            "成功"
        } else {
            "失败"
        }
    );

    if !deposit_receipt.status() {
        return Err(eyre!("存款交易失败"));
    }

    // 解析事件日志
    for log in deposit_receipt.inner.logs() {
        // 转换 RPC 日志为原语日志
        let primitive_log = alloy::primitives::Log {
            address: log.address(),
            data: log.data().clone(),
        };

        if let Ok(event) = PaymentVault::DepositReceived::decode_log(&primitive_log) {
            println!("存款事件详情:");
            println!("  付款方: {}", event.payer);
            println!("  订单ID: {}", event.orderId);
            println!("  金额: {} USDC", format_usdc_amount(event.amount));
        }
    }

    println!("✅ 存款操作完成!");
    Ok(())
}

/// 执行使用 permit 签名的存款操作
pub async fn execute_deposit_with_permit(
    rpc_url: String,
    private_key: String,
    contract_address: String,
    order_id: String,
    amount: u64,
    deadline: u64,
) -> Result<()> {
    println!("=== 执行 Permit 存款操作 ===");

    // 解析参数
    let contract_addr = parse_address(&contract_address)?;
    let order_id_bytes = parse_order_id(&order_id)?;
    let amount_u256 = U256::from(amount);

    // 创建提供者
    let provider = create_provider_with_wallet(&rpc_url, &private_key).await?;

    // 获取钱包地址
    let wallet_address = provider.get_accounts().await?[0];
    println!("钱包地址: {}", wallet_address);

    // 创建合约实例
    let vault_contract = PaymentVault::new(contract_addr, &provider);

    // 获取USDC代币地址
    let usdc_address = vault_contract.usdcToken().call().await?;
    let usdc_contract = USDC::new(usdc_address, &provider);

    println!("PaymentVault 合约地址: {}", contract_addr);
    println!("USDC 代币地址: {}", usdc_address);
    println!("订单ID: {}", order_id);
    println!("存款金额: {} USDC", format_usdc_amount(amount_u256));
    println!("Permit 截止时间: {}", deadline);

    // 检查USDC余额
    let balance = usdc_contract.balanceOf(wallet_address).call().await?;
    println!("当前USDC余额: {} USDC", format_usdc_amount(balance));

    if balance < amount_u256 {
        return Err(eyre!(
            "USDC余额不足。需要: {} USDC, 当前: {} USDC",
            format_usdc_amount(amount_u256),
            format_usdc_amount(balance)
        ));
    }

    // 获取 permit 所需的信息
    let nonce = usdc_contract.nonces(wallet_address).call().await?;
    let domain_separator = usdc_contract.DOMAIN_SEPARATOR().call().await?;

    println!("当前 nonce: {}", nonce);
    println!("Domain separator: {}", domain_separator);

    // 创建 permit 签名
    // 这里简化处理，实际应该使用 EIP-712 标准签名
    // 为了示例，我们使用固定的 v, r, s 值
    // 在生产环境中，应该正确实现 EIP-712 签名
    let v = 27u8;
    let r = FixedBytes::from([0u8; 32]);
    let s = FixedBytes::from([0u8; 32]);

    println!("⚠️  注意: 这是一个简化的示例。生产环境中需要正确实现 EIP-712 签名。");

    // 执行带 permit 的存款
    println!("正在执行 permit 存款...");
    let deposit_tx = vault_contract
        .depositWithPermit(order_id_bytes, amount_u256, U256::from(deadline), v, r, s)
        .send()
        .await?;
    let deposit_receipt = deposit_tx.get_receipt().await?;

    println!("存款交易哈希: {}", deposit_receipt.transaction_hash);
    println!(
        "存款交易状态: {}",
        if deposit_receipt.status() {
            "成功"
        } else {
            "失败"
        }
    );

    if !deposit_receipt.status() {
        return Err(eyre!("存款交易失败"));
    }

    // 解析事件日志
    for log in deposit_receipt.inner.logs() {
        // 转换 RPC 日志为原语日志
        let primitive_log = alloy::primitives::Log {
            address: log.address(),
            data: log.data().clone(),
        };

        if let Ok(event) = PaymentVault::DepositReceived::decode_log(&primitive_log) {
            println!("存款事件详情:");
            println!("  付款方: {}", event.payer);
            println!("  订单ID: {}", event.orderId);
            println!("  金额: {} USDC", format_usdc_amount(event.amount));
        }
    }

    println!("✅ Permit 存款操作完成!");
    Ok(())
}

/// 执行提取操作（仅限所有者）
pub async fn execute_withdraw(
    rpc_url: String,
    private_key: String,
    contract_address: String,
) -> Result<()> {
    println!("=== 执行提取操作 ===");

    // 解析参数
    let contract_addr = parse_address(&contract_address)?;

    // 创建提供者
    let provider = create_provider_with_wallet(&rpc_url, &private_key).await?;

    // 获取钱包地址
    let wallet_address = provider.get_accounts().await?[0];
    println!("钱包地址: {}", wallet_address);

    // 创建合约实例
    let vault_contract = PaymentVault::new(contract_addr, &provider);

    // 获取USDC代币地址
    let usdc_address = vault_contract.usdcToken().call().await?;
    let usdc_contract = USDC::new(usdc_address, &provider);

    println!("PaymentVault 合约地址: {}", contract_addr);
    println!("USDC 代币地址: {}", usdc_address);

    // 检查是否为所有者
    let owner = vault_contract.owner().call().await?;
    if wallet_address != owner {
        return Err(eyre!(
            "只有合约所有者可以执行提取操作。当前钱包: {}, 所有者: {}",
            wallet_address,
            owner
        ));
    }

    // 获取合约余额
    let contract_balance = usdc_contract.balanceOf(contract_addr).call().await?;
    println!(
        "合约USDC余额: {} USDC",
        format_usdc_amount(contract_balance)
    );

    if contract_balance == U256::ZERO {
        return Err(eyre!("合约中没有可提取的资金"));
    }

    // 执行提取
    println!("正在执行提取...");
    let withdraw_tx = vault_contract.withdraw().send().await?;
    let withdraw_receipt = withdraw_tx.get_receipt().await?;

    println!("提取交易哈希: {}", withdraw_receipt.transaction_hash);
    println!(
        "提取交易状态: {}",
        if withdraw_receipt.status() {
            "成功"
        } else {
            "失败"
        }
    );

    if !withdraw_receipt.status() {
        return Err(eyre!("提取交易失败"));
    }

    // 解析事件日志
    for log in withdraw_receipt.inner.logs() {
        // 转换 RPC 日志为原语日志
        let primitive_log = alloy::primitives::Log {
            address: log.address(),
            data: log.data().clone(),
        };

        if let Ok(event) = PaymentVault::WithdrawCompleted::decode_log(&primitive_log) {
            println!("提取事件详情:");
            println!("  所有者: {}", event.owner);
            println!("  金额: {} USDC", format_usdc_amount(event.amount));
        }
    }

    println!("✅ 提取操作完成!");
    Ok(())
}
