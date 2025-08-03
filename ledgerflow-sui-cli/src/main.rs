use std::path::PathBuf;

use clap::Parser;
use eyre::{Context, Result};
use serde_json::json;
use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, fmt};

mod cli;
mod client;
mod config;

use cli::{Cli, Commands, OutputFormat};
use client::VaultClient;
use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    // Install color-eyre for better error handling
    color_eyre::install()?;

    let args = Cli::parse();

    // Initialize tracing
    let log_level = if args.verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("ledgerflow_sui_cli={log_level},sui_sdk=warn")));

    fmt().with_env_filter(filter).with_target(false).init();

    // Process commands
    match args.command {
        Commands::Init { path, force } => handle_init(path, force).await,
        Commands::Deposit {
            order_id,
            amount,
            dry_run,
        } => {
            let config = load_config(args.config)?;
            handle_deposit(config, order_id, amount, dry_run, args.output).await
        }
        Commands::Withdraw {
            recipient,
            amount,
            all,
            dry_run,
        } => {
            let config = load_config(args.config)?;
            handle_withdraw(config, recipient, amount, all, dry_run, args.output).await
        }
        Commands::Info { include_account } => {
            let config = load_config(args.config)?;
            handle_info(config, include_account, args.output).await
        }
        Commands::Account { show_private } => {
            let config = load_config(args.config)?;
            handle_account(config, show_private, args.output).await
        }
    }
}

async fn handle_init(path: PathBuf, force: bool) -> Result<()> {
    if path.exists() && !force {
        eyre::bail!(
            "Configuration file already exists at {}. Use --force to overwrite.",
            path.display()
        );
    }

    Config::create_sample(&path)?;
    info!("Configuration file created at: {}", path.display());

    println!("✅ Configuration file created successfully!");
    println!(
        "📝 Please edit {} to configure your settings:",
        path.display()
    );
    println!("   - Set your private key in account.private_key");
    println!("   - Set the vault package ID in vault.package_id");
    println!("   - Set the vault object ID in vault.vault_object_id");
    println!("   - Adjust network settings if needed");

    Ok(())
}

async fn handle_deposit(
    config: Config,
    order_id: String,
    amount: u64,
    dry_run: bool,
    output_format: OutputFormat,
) -> Result<()> {
    let mut client = VaultClient::new(
        config.network.rpc_url.clone(),
        config.account.private_key.clone(),
        config.vault.package_id.clone(),
        config.vault.vault_object_id.clone(),
        config.vault.usdc_type.clone(),
        config.transaction.gas_budget,
    )?;

    if dry_run {
        info!("🔍 Dry run mode - transaction will not be submitted");

        let result = json!({
            "operation": "deposit",
            "order_id": order_id,
            "amount": amount,
            "account": client.account_address(),
            "dry_run": true,
            "status": "simulated"
        });

        print_output(&result, &output_format);
        return Ok(());
    }

    info!(
        "💰 Processing deposit: {} units with order_id: {}",
        amount, order_id
    );

    match client.deposit(amount, order_id.as_bytes().to_vec()).await {
        Ok(tx_result) => {
            let result = json!({
                "operation": "deposit",
                "order_id": order_id,
                "amount": amount,
                "transaction_hash": tx_result.hash,
                "gas_used": tx_result.gas_used,
                "timestamp": tx_result.timestamp,
                "status": if tx_result.success { "success" } else { "failed" }
            });

            print_output(&result, &output_format);

            if matches!(output_format, OutputFormat::Pretty) {
                println!("✅ Deposit successful!");
                println!("📦 Order ID: {order_id}");
                println!("💎 Amount: {amount} units");
                println!("🔗 Transaction: {}", tx_result.hash);
            }
        }
        Err(e) => {
            error!("Failed to deposit: {:#}", e);

            let result = json!({
                "operation": "deposit",
                "order_id": order_id,
                "amount": amount,
                "account": client.account_address(),
                "status": "error",
                "error": format!("{:#}", e)
            });

            print_output(&result, &output_format);
            return Err(e);
        }
    }

    Ok(())
}

async fn handle_withdraw(
    config: Config,
    recipient: String,
    amount: u64,
    all: bool,
    dry_run: bool,
    output_format: OutputFormat,
) -> Result<()> {
    let mut client = VaultClient::new(
        config.network.rpc_url.clone(),
        config.account.private_key.clone(),
        config.vault.package_id.clone(),
        config.vault.vault_object_id.clone(),
        config.vault.usdc_type.clone(),
        config.transaction.gas_budget,
    )?;

    if dry_run {
        info!("🔍 Dry run mode - transaction will not be submitted");

        let amount_str = if all {
            "all".to_string()
        } else {
            amount.to_string()
        };
        let result = json!({
            "operation": if all { "withdraw_all" } else { "withdraw" },
            "recipient": recipient,
            "amount": amount_str,
            "account": client.account_address(),
            "dry_run": true,
            "status": "simulated"
        });

        print_output(&result, &output_format);
        return Ok(());
    }

    let tx_result = if all {
        info!("💸 Withdrawing all funds from vault to: {}", recipient);
        client.withdraw_all(recipient.clone()).await?
    } else {
        info!(
            "💸 Withdrawing {} units from vault to: {}",
            amount, recipient
        );
        client.withdraw(amount, recipient.clone()).await?
    };

    let amount_str = if all {
        "all".to_string()
    } else {
        amount.to_string()
    };
    let result = json!({
        "operation": if all { "withdraw_all" } else { "withdraw" },
        "recipient": recipient,
        "amount": amount_str,
        "transaction_hash": tx_result.hash,
        "account": client.account_address(),
        "status": "success"
    });

    print_output(&result, &output_format);

    if matches!(output_format, OutputFormat::Pretty) {
        println!("✅ Withdrawal successful!");
        println!("📤 Recipient: {recipient}");
        if !all {
            println!("💎 Amount: {amount} units");
        } else {
            println!("💎 Amount: All available funds");
        }
        println!("🔗 Transaction: {}", tx_result.hash);
    }

    Ok(())
}

async fn handle_info(
    config: Config,
    include_account: bool,
    output_format: OutputFormat,
) -> Result<()> {
    let client = VaultClient::new(
        config.network.rpc_url.clone(),
        config.account.private_key.clone(),
        config.vault.package_id.clone(),
        config.vault.vault_object_id.clone(),
        config.vault.usdc_type.clone(),
        config.transaction.gas_budget,
    )?;

    info!("📊 Fetching vault information...");

    let vault_info = client.get_vault_info().await?;

    let mut result = json!({
        "vault": {
            "address": vault_info["vault_address"],
            "balance": vault_info["total_deposit"],
            "owner": vault_info["owner"],
            "created_at": vault_info["created_at"]
        }
    });

    if include_account {
        let account_address = client.account_address();
        let account_balance = client.get_balance(&account_address).await?;
        let sui_balance = client.get_sui_balance(&account_address).await?;
        result["account"] = json!({
            "address": account_address,
            "usdc_balance": account_balance,
            "sui_balance": sui_balance
        });
    }

    print_output(&result, &output_format);

    if matches!(output_format, OutputFormat::Pretty) {
        println!("📊 Vault Information");
        println!("━━━━━━━━━━━━━━━━━━━━");
        println!("🏦 Vault Address: {}", vault_info["vault_address"]);
        println!("💰 Balance: {} units", vault_info["total_deposit"]);
        println!("👑 Owner: {}", vault_info["owner"]);
        println!("📅 Created At: {}", vault_info["created_at"]);

        if include_account {
            let account_address = client.account_address();
            let account_balance = client.get_balance(&account_address).await?;
            let sui_balance = client.get_sui_balance(&account_address).await?;
            println!("\n👤 Account Information");
            println!("━━━━━━━━━━━━━━━━━━━━━");
            println!("📍 Address: {account_address}");
            println!("💎 USDC Balance: {account_balance} units");
            println!("⛽ SUI Balance: {sui_balance} MIST");
        }
    }

    Ok(())
}

async fn handle_account(
    config: Config,
    show_private: bool,
    output_format: OutputFormat,
) -> Result<()> {
    let client = VaultClient::new(
        config.network.rpc_url.clone(),
        config.account.private_key.clone(),
        config.vault.package_id.clone(),
        config.vault.vault_object_id.clone(),
        config.vault.usdc_type.clone(),
        config.transaction.gas_budget,
    )?;

    info!("👤 Fetching account information...");

    let account_address = client.account_address();
    let account_balance = client.get_balance(&account_address).await?;
    let sui_balance = client.get_sui_balance(&account_address).await?;

    let mut result = json!({
        "address": account_address,
        "usdc_balance": account_balance,
        "sui_balance": sui_balance,
    });

    if show_private {
        warn!("⚠️  Displaying private key information");
        result["private_key"] = json!(config.account.private_key);
    }

    print_output(&result, &output_format);

    if matches!(output_format, OutputFormat::Pretty) {
        println!("👤 Account Information");
        println!("━━━━━━━━━━━━━━━━━━━━");
        println!("📍 Address: {account_address}");
        println!("💎 USDC Balance: {account_balance} units");
        println!("⛽ SUI Balance: {sui_balance} MIST");

        if show_private {
            println!("🔐 Private Key: {}", config.account.private_key);
            println!("⚠️  Keep your private key secure and never share it!");
        }
    }

    Ok(())
}

fn load_config(config_path: Option<PathBuf>) -> Result<Config> {
    Config::load(config_path).context("Failed to load configuration")
}

fn print_output(value: &serde_json::Value, format: &OutputFormat) {
    match format {
        OutputFormat::Pretty => {
            // Pretty output is handled in each command handler
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(value).expect("Failed to serialize JSON")
            );
        }
        OutputFormat::Compact => {
            println!(
                "{}",
                serde_json::to_string(value).expect("Failed to serialize JSON")
            );
        }
    }
}
