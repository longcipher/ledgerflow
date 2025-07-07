use std::str::FromStr;

use alloy::{
    network::EthereumWallet,
    primitives::{Address, FixedBytes, U256},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
};
use eyre::{Result, eyre};

/// 解析十六进制字符串为地址
pub fn parse_address(addr_str: &str) -> Result<Address> {
    let addr_str = if addr_str.starts_with("0x") {
        &addr_str[2..]
    } else {
        addr_str
    };

    Address::from_str(addr_str).map_err(|e| eyre!("无效的地址格式: {}", e))
}

/// 解析十六进制字符串为 FixedBytes<32> (用于订单ID)
pub fn parse_order_id(order_id_str: &str) -> Result<FixedBytes<32>> {
    let order_id_str = if order_id_str.starts_with("0x") {
        &order_id_str[2..]
    } else {
        order_id_str
    };

    if order_id_str.len() != 64 {
        return Err(eyre!("订单ID必须是32字节(64个十六进制字符)"));
    }

    FixedBytes::from_str(order_id_str).map_err(|e| eyre!("无效的订单ID格式: {}", e))
}

/// 解析私钥并创建签名者
pub fn parse_private_key(private_key_str: &str) -> Result<PrivateKeySigner> {
    let private_key_str = if private_key_str.starts_with("0x") {
        &private_key_str[2..]
    } else {
        private_key_str
    };

    PrivateKeySigner::from_str(private_key_str).map_err(|e| eyre!("无效的私钥格式: {}", e))
}

/// 创建带钱包的提供者
pub async fn create_provider_with_wallet(
    rpc_url: &str,
    private_key: &str,
) -> Result<impl alloy::providers::Provider + Clone> {
    let signer = parse_private_key(private_key)?;
    let wallet = EthereumWallet::from(signer);

    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect(rpc_url)
        .await?;

    Ok(provider)
}

/// 格式化金额显示 (假设USDC有6位小数)
pub fn format_usdc_amount(amount: U256) -> String {
    let amount_u64 = amount.to::<u64>();
    let whole = amount_u64 / 1_000_000;
    let decimal = amount_u64 % 1_000_000;
    format!("{}.{:06}", whole, decimal)
}

/// 从字符串解析USDC金额 (例如 "1.5" -> 1500000)
#[allow(dead_code)]
pub fn parse_usdc_amount(amount_str: &str) -> Result<U256> {
    let amount: f64 = amount_str.parse()?;
    let amount_scaled = (amount * 1_000_000.0) as u64;
    Ok(U256::from(amount_scaled))
}
