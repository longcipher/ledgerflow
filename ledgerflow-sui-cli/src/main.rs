use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use clap::Parser;
use eyre::{Context, Result};
use serde_json::json;
use tracing::{error, info, warn};
use tracing_subscriber::{EnvFilter, fmt};

mod cli;
mod client;
mod config;

use cli::{Cli, Commands, OutputFormat};
use client::{IntentSignedTransaction, VaultClient};
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
        Commands::IntentTransfer {
            recipient,
            amount,
            order_id,
            facilitator_url,
            dry_run,
            test_settle,
        } => {
            let config = load_config(args.config)?;
            handle_intent_transfer(
                config,
                recipient,
                amount,
                Some(order_id),
                Some(facilitator_url),
                test_settle,
                dry_run,
                args.output,
            )
            .await
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

    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("toml"))
        .unwrap_or(false)
    {
        println!(
            "   - Set your private key in [account].private_key or use SUI_PRIVATE_KEY env var"
        );
        println!("   - Set the vault package ID in [vault].package_id");
        println!("   - Set the vault object ID in [vault].vault_object_id");
        println!("   - Adjust network settings in [network] if needed");
    } else {
        println!("   - Set your private key in account.private_key");
        println!("   - Set the vault package ID in vault.package_id");
        println!("   - Set the vault object ID in vault.vault_object_id");
        println!("   - Adjust network settings if needed");
    }

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
        config.account.address.clone(),
    )
    .await?;

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
        config.account.address.clone(),
    )
    .await?;

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
        config.account.address.clone(),
    )
    .await?;

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
        config.account.address.clone(),
    )
    .await?;

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

#[allow(clippy::too_many_arguments)]
async fn handle_intent_transfer(
    config: Config,
    recipient: String,
    amount: u64,
    order_id: Option<String>,
    facilitator_url: Option<String>,
    test_settle: bool,
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
        config.account.address.clone(),
    )
    .await?;

    let final_order_id = order_id.unwrap_or_else(|| {
        format!(
            "order_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Failed to get current time")
                .as_secs()
        )
    });

    info!(
        "🔐 Creating intent-signed transfer: {} units to {} (order: {})",
        amount, recipient, final_order_id
    );

    // Create the intent-signed transaction
    let intent_tx = client
        .create_intent_transfer(recipient.clone(), amount, final_order_id.clone())
        .await?;

    // Always print the complete intent signature for verification
    info!("🔐 Intent Signature Generated:");
    println!(
        "📋 Complete Intent Signature: {}",
        intent_tx.intent_signature
    );
    println!("🔑 Public Key: {}", intent_tx.public_key);
    println!("📍 Sender Address: {}", intent_tx.sender_address);
    println!("📤 Recipient: {recipient}");
    println!("💎 Amount: {amount} units");
    println!("📦 Order ID: {final_order_id}");

    if dry_run {
        info!("� Dry run mode - transaction will not be sent to facilitator");
        println!("✅ Intent-signed transaction created successfully (dry-run)!");
        return Ok(());
    }

    // Send to facilitator for verification if URL is provided
    let (verify_result, settle_result) = if let Some(ref facilitator_url_ref) = facilitator_url {
        info!(
            "🚀 Sending transaction to facilitator for verification: {}",
            facilitator_url_ref
        );

        let verify_result = match send_to_facilitator(&intent_tx, facilitator_url_ref).await {
            Ok(result) => {
                println!("✅ Intent signature successfully verified by facilitator!");
                Some(result)
            }
            Err(e) => {
                if test_settle {
                    println!("⚠️ Verification failed, but proceeding to test settle: {e}");
                    None
                } else {
                    return Err(e);
                }
            }
        };

        // Test settle flow if requested
        let settle_result = if test_settle || verify_result.is_some() {
            info!("🔄 Testing settle flow...");
            match send_to_settle(&intent_tx, facilitator_url_ref).await {
                Ok(result) => {
                    println!("✅ Settlement successful!");
                    Some(result)
                }
                Err(e) => {
                    println!("❌ Settlement failed: {e}");
                    None
                }
            }
        } else {
            None
        };

        (verify_result, settle_result)
    } else {
        println!("✅ Intent-signed transaction created successfully!");
        (None, None)
    };

    let result = json!({
        "operation": "intent_transfer",
        "recipient": recipient,
        "amount": amount,
        "order_id": final_order_id,
        "transaction": {
            "signature": intent_tx.intent_signature,
            "public_key": intent_tx.public_key,
            "address": intent_tx.sender_address
        },
        "facilitator": {
            "url": facilitator_url,
            "verify_response": verify_result,
            "settle_response": settle_result
        },
        "status": "verified_and_submitted"
    });

    print_output(&result, &output_format);

    if matches!(output_format, OutputFormat::Pretty) {
        println!("✅ Intent-signed transaction processed successfully!");
        if let Some(ref facilitator_url_ref) = facilitator_url {
            println!("🔗 Facilitator: {facilitator_url_ref}");
        }

        if let Some(settle_resp) = &settle_result
            && let Some(tx_hash) = settle_resp.get("transaction")
        {
            println!("📋 Transaction Hash: {tx_hash}");
        }
    }

    Ok(())
}

async fn send_to_facilitator(
    intent_tx: &IntentSignedTransaction,
    facilitator_url: &str,
) -> Result<serde_json::Value> {
    let client = reqwest::Client::new();

    // Create the facilitator payload following the x402 protocol format
    let payload = json!({
        "x402Version": 1,
        "paymentPayload": {
            "x402Version": 1,
            "scheme": "exact",
            "network": "sui-testnet",
            "payload": {
                "signature": intent_tx.intent_signature,
                "authorization": {
                    "from": intent_tx.sender_address,
                    "to": intent_tx.recipient,
                    "value": intent_tx.amount_str.clone(),
                    "validAfter": intent_tx.valid_after,
                    "validBefore": intent_tx.valid_before,
                    "nonce": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "coinType": "0x2::sui::SUI"
                },
                "gasBudget": 10000000
            }
        },
        "paymentRequirements": {
            "scheme": "exact",
            "network": "sui-testnet",
            "maxAmountRequired": intent_tx.amount_str.clone(),
            "resource": "https://example.com/ledgerflow-cli-payment",
            "description": format!("Payment from CLI: {} units to {}", intent_tx.amount_str, intent_tx.recipient),
            "mimeType": "application/json",
            "payTo": intent_tx.recipient,
            "maxTimeoutSeconds": 3600,
            "asset": "0xca66c8d82ed90bd31190db432124459e210cdec15cdd6aff20f3e6cb6decdf49",
            "extra": null
        }
    });

    info!("📤 Sending payload to facilitator:");
    println!(
        "🔍 Facilitator Payload:\n{}",
        serde_json::to_string_pretty(&payload)?
    );

    // Send verify request to facilitator
    let verify_url = format!("{facilitator_url}/verify");
    info!("🔗 Verify URL: {}", verify_url);

    let verify_response = client
        .post(&verify_url)
        .json(&payload)
        .send()
        .await
        .context("Failed to send verify request to facilitator")?;

    let status = verify_response.status();
    info!("📡 Facilitator verify response status: {}", status);

    if !status.is_success() {
        let error_text = verify_response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        println!("❌ Facilitator verify failed with status {status}: {error_text}");
        eyre::bail!("Facilitator verify failed: {}", error_text);
    }

    let verify_result: serde_json::Value = verify_response
        .json()
        .await
        .context("Failed to parse verify response")?;

    info!("📋 Facilitator verify response:");
    println!(
        "✅ Verify Response:\n{}",
        serde_json::to_string_pretty(&verify_result)?
    );

    let is_valid = verify_result
        .get("isValid")
        .unwrap_or(&json!(false))
        .as_bool()
        .unwrap_or(false);

    if !is_valid {
        println!("❌ Transaction verification failed!");
        eyre::bail!("Transaction verification failed: {:?}", verify_result);
    }

    println!("✅ Intent signature successfully verified by facilitator!");

    Ok(verify_result)
}

async fn send_to_settle(
    intent_tx: &IntentSignedTransaction,
    facilitator_url: &str,
) -> Result<serde_json::Value> {
    let client = reqwest::Client::new();

    // Create the same payload structure as for verify
    let payload = json!({
        "x402Version": 1,
        "paymentPayload": {
            "x402Version": 1,
            "scheme": "exact",
            "network": "sui-testnet",
            "payload": {
                "signature": intent_tx.intent_signature,
                "authorization": {
                    "from": intent_tx.sender_address,
                    "to": intent_tx.recipient,
                    "value": intent_tx.amount_str.clone(),
                    "validAfter": intent_tx.valid_after,
                    "validBefore": intent_tx.valid_before,
                    "nonce": "0x0000000000000000000000000000000000000000000000000000000000000001",
                    "coinType": "0x2::sui::SUI"
                },
                "gasBudget": 10000000
            }
        },
        "paymentRequirements": {
            "scheme": "exact",
            "network": "sui-testnet",
            "maxAmountRequired": intent_tx.amount_str.clone(),
            "resource": "https://example.com/ledgerflow-cli-payment",
            "description": format!("Settlement from CLI: {} units to {}", intent_tx.amount_str, intent_tx.recipient),
            "mimeType": "application/json",
            "payTo": intent_tx.recipient,
            "maxTimeoutSeconds": 3600,
            "asset": "0xca66c8d82ed90bd31190db432124459e210cdec15cdd6aff20f3e6cb6decdf49",
            "extra": null
        }
    });

    info!("🔄 Sending payload to facilitator settle endpoint:");
    println!(
        "🔄 Settle Payload:\n{}",
        serde_json::to_string_pretty(&payload)?
    );

    // Send settle request to facilitator
    let settle_url = format!("{facilitator_url}/settle");
    info!("🔗 Settle URL: {}", settle_url);

    let settle_response = client
        .post(&settle_url)
        .json(&payload)
        .send()
        .await
        .context("Failed to send settle request to facilitator")?;

    let status = settle_response.status();
    info!("📡 Facilitator settle response status: {}", status);

    if !status.is_success() {
        let error_text = settle_response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        println!("❌ Facilitator settle failed with status {status}: {error_text}");
        eyre::bail!("Facilitator settle failed: {}", error_text);
    }

    let settle_result: serde_json::Value = settle_response
        .json()
        .await
        .context("Failed to parse settle response")?;

    info!("📋 Facilitator settle response:");
    println!(
        "✅ Settle Response:\n{}",
        serde_json::to_string_pretty(&settle_result)?
    );

    let success = settle_result
        .get("success")
        .unwrap_or(&json!(false))
        .as_bool()
        .unwrap_or(false);

    if !success {
        println!("❌ Transaction settlement failed!");
        eyre::bail!("Transaction settlement failed: {:?}", settle_result);
    }

    println!("✅ Transaction successfully settled by facilitator!");

    Ok(settle_result)
}
