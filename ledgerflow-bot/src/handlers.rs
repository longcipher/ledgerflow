use teloxide::{dispatching::UpdateHandler, prelude::*};
use tracing::{error, info};

use crate::{
    config::Config,
    database::Database,
    error::{BotError, BotResult},
    models::{CreateOrderRequest, User},
    services::BalancerService,
    wallet,
};

pub type BotState = std::sync::Arc<AppState>;

pub struct AppState {
    pub database: Database,
    pub config: Config,
    pub balancer: BalancerService,
}

pub async fn create_handler(
    _bot: Bot,
    database: Database,
    config: Config,
) -> BotResult<UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>>> {
    let balancer = BalancerService::new(&config);

    let state = std::sync::Arc::new(AppState {
        database,
        config,
        balancer,
    });

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint({
            let state = state.clone();
            move |bot: Bot, msg: Message| {
                let state = state.clone();
                async move {
                    handle_message(bot, msg, state).await.map_err(|e| {
                        Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>
                    })
                }
            }
        }))
        .branch(Update::filter_callback_query().endpoint({
            let state = state.clone();
            move |bot: Bot, callback: CallbackQuery| {
                let state = state.clone();
                async move {
                    handle_callback(bot, callback, state).await.map_err(|e| {
                        Box::new(e) as Box<dyn std::error::Error + Send + Sync + 'static>
                    })
                }
            }
        }));

    Ok(handler)
}

async fn handle_message(
    bot: Bot,
    msg: Message,
    state: BotState,
) -> Result<(), teloxide::RequestError> {
    info!(
        "Received message from user: {:?}",
        msg.from.as_ref().map(|u| u.id)
    );

    // Ensure user exists in database
    if let Some(user) = &msg.from {
        let db_user = User {
            id: uuid::Uuid::new_v4(),
            telegram_id: user.id.0 as i64,
            username: user.username.clone(),
            first_name: Some(user.first_name.clone()),
            last_name: user.last_name.clone(),
            evm_address: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        if let Err(e) = state.database.create_user(&db_user).await {
            error!("Failed to create user: {}", e);
        }
    }

    if let Some(text) = msg.text() {
        let chat_id = msg.chat.id;
        let result = match text {
            "/start" => handle_start(bot.clone(), msg.clone(), state.clone()).await,
            "/help" => handle_help(bot.clone(), msg.clone(), state.clone()).await,
            "/balance" => handle_balance(bot.clone(), msg.clone(), state.clone()).await,
            "/wallet" => handle_wallet(bot.clone(), msg.clone(), state.clone()).await,
            "/generate_wallet" => {
                handle_generate_wallet(bot.clone(), msg.clone(), state.clone()).await
            }
            text if text.starts_with("/pay ") => {
                handle_pay(bot.clone(), msg.clone(), state.clone(), text).await
            }
            text if text.starts_with("/bind ") => {
                handle_bind_address(bot.clone(), msg.clone(), state.clone(), text).await
            }
            _ => handle_unknown_command(bot.clone(), msg.clone(), state.clone()).await,
        };

        if let Err(e) = result {
            error!("Handler error: {}", e);
            let _ = bot
                .send_message(chat_id, "Sorry, something went wrong. Please try again.")
                .await;
        }
    }

    Ok(())
}

async fn handle_callback(
    bot: Bot,
    callback: CallbackQuery,
    state: BotState,
) -> Result<(), teloxide::RequestError> {
    let callback_id = callback.id.clone();

    if let Some(ref data) = callback.data {
        let result = match data.as_str() {
            "generate_wallet" => {
                handle_generate_wallet_callback(bot.clone(), callback, state.clone()).await
            }
            "check_balance" => handle_balance_callback(bot.clone(), callback, state.clone()).await,
            _ => {
                bot.answer_callback_query(callback_id.clone()).await?;
                Ok(())
            }
        };

        if let Err(e) = result {
            error!("Callback handler error: {}", e);
            let _ = bot.answer_callback_query(callback_id).await;
        }
    } else {
        bot.answer_callback_query(callback_id).await?;
    }

    Ok(())
}

async fn handle_start(bot: Bot, msg: Message, _state: BotState) -> BotResult<()> {
    let welcome_text = "🚀 Welcome to LedgerFlow Bot!\n\n\
        I can help you:\n\
        • 💳 Create payment requests\n\
        • 💰 Check your balance\n\
        • 🔗 Bind your EVM address\n\
        • 👛 Generate a new wallet\n\n\
        Use /help to see all available commands.";

    bot.send_message(msg.chat.id, welcome_text).await?;
    Ok(())
}

async fn handle_help(bot: Bot, msg: Message, _state: BotState) -> BotResult<()> {
    let help_text = "📋 Available Commands:\n\n\
        /start - Start the bot\n\
        /help - Show this help message\n\
        /balance - Check your balance\n\
        /wallet - Show your wallet info\n\
        /generate_wallet - Generate a new wallet\n\
        /pay <amount> - Create a payment request\n\
        /bind <address> - Bind your EVM address\n\n\
        Example: /pay 10.5";

    bot.send_message(msg.chat.id, help_text).await?;
    Ok(())
}

async fn handle_balance(bot: Bot, msg: Message, state: BotState) -> BotResult<()> {
    let user_id = msg
        .from
        .as_ref()
        .map(|u| u.id.0 as i64)
        .ok_or_else(|| BotError::Config("User not found".to_string()))?;

    match state.balancer.get_balance(&user_id.to_string()).await {
        Ok(balance) => {
            let balance_text = format!(
                "💰 Your Balance:\n\n\
                Total: {} USDC\n\
                Account: {}\n\n\
                Use /pay <amount> to create a payment request",
                balance.balance, balance.account_id
            );
            bot.send_message(msg.chat.id, balance_text).await?;
        }
        Err(e) => {
            error!("Failed to get balance: {}", e);
            bot.send_message(
                msg.chat.id,
                "❌ Failed to retrieve balance. Please try again later.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn handle_wallet(bot: Bot, msg: Message, state: BotState) -> BotResult<()> {
    let user_id = msg
        .from
        .as_ref()
        .map(|u| u.id.0 as i64)
        .ok_or_else(|| BotError::Config("User not found".to_string()))?;

    match state
        .database
        .get_user_by_telegram_id(user_id)
        .await
        .map_err(BotError::from)?
    {
        Some(user) => {
            let wallet_text = if let Some(address) = user.evm_address {
                format!(
                    "👛 Your Wallet:\n\n\
                    Address: `{address}`\n\n\
                    Use /bind <address> to change your address"
                )
            } else {
                "👛 No wallet address bound\n\n\
                Use /bind <address> to bind your address\n\
                Or use /generate_wallet to create a new one"
                    .to_string()
            };

            bot.send_message(msg.chat.id, wallet_text)
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await?;
        }
        None => {
            bot.send_message(msg.chat.id, "❌ User not found").await?;
        }
    }

    Ok(())
}

async fn handle_generate_wallet(bot: Bot, msg: Message, _state: BotState) -> BotResult<()> {
    match wallet::generate_wallet().await {
        Ok(wallet) => {
            let wallet_text = format!(
                "🆕 Generated New Wallet:\n\n\
                Address: `{}`\n\
                Private Key: `{}`\n\n\
                ⚠️ **IMPORTANT**: Keep your private key secure!\n\
                Use /bind {} to bind this address to your account",
                wallet.address, wallet.private_key, wallet.address
            );

            bot.send_message(msg.chat.id, wallet_text)
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await?;
        }
        Err(e) => {
            error!("Failed to generate wallet: {}", e);
            bot.send_message(
                msg.chat.id,
                "❌ Failed to generate wallet. Please try again.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn handle_pay(bot: Bot, msg: Message, state: BotState, text: &str) -> BotResult<()> {
    let user_id = msg
        .from
        .as_ref()
        .map(|u| u.id.0 as i64)
        .ok_or_else(|| BotError::Config("User not found".to_string()))?;

    // Parse amount from command
    let amount_str = text.strip_prefix("/pay ").unwrap_or("").trim();

    if amount_str.is_empty() {
        bot.send_message(
            msg.chat.id,
            "❌ Please specify an amount\nExample: /pay 10.5",
        )
        .await?;
        return Ok(());
    }

    // Validate amount
    if amount_str.parse::<f64>().is_err() {
        bot.send_message(msg.chat.id, "❌ Invalid amount format\nExample: /pay 10.5")
            .await?;
        return Ok(());
    }

    let request = CreateOrderRequest {
        account_id: user_id.to_string(),
        amount: amount_str.to_string(),
        token_address: state.config.blockchain.payment_vault_address.clone(),
    };

    match state.balancer.create_order(request).await {
        Ok(order) => {
            let payment_text = format!(
                "💳 Payment Request Created:\n\n\
                Order ID: `{}`\n\
                Amount: {} USDC\n\
                Payment Address: `{}`\n\
                Chain: Unichain Sepolia\n\n\
                Send the exact amount to the payment address with the Order ID in the transaction data.",
                order.order_id, order.amount, order.payment_address
            );

            bot.send_message(msg.chat.id, payment_text)
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await?;
        }
        Err(e) => {
            error!("Failed to create order: {}", e);
            bot.send_message(
                msg.chat.id,
                "❌ Failed to create payment request. Please try again.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn handle_bind_address(bot: Bot, msg: Message, state: BotState, text: &str) -> BotResult<()> {
    let user_id = msg
        .from
        .as_ref()
        .map(|u| u.id.0 as i64)
        .ok_or_else(|| BotError::Config("User not found".to_string()))?;

    let address = text.strip_prefix("/bind ").unwrap_or("").trim();

    if address.is_empty() {
        bot.send_message(msg.chat.id, "❌ Please specify an address\nExample: /bind 0x742d35Cc6634C0532925a3b8D4fd6c4d4d61ddD6").await?;
        return Ok(());
    }

    // Validate address
    if !wallet::validate_address(address).map_err(BotError::from)? {
        bot.send_message(msg.chat.id, "❌ Invalid address format")
            .await?;
        return Ok(());
    }

    let formatted_address = wallet::format_address(address);

    match state
        .database
        .update_user_evm_address(user_id, &formatted_address)
        .await
        .map_err(BotError::from)
    {
        Ok(_) => {
            let success_text = format!(
                "✅ Address bound successfully!\n\n\
                Your address: `{formatted_address}`"
            );
            bot.send_message(msg.chat.id, success_text)
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await?;
        }
        Err(e) => {
            error!("Failed to bind address: {}", e);
            bot.send_message(msg.chat.id, "❌ Failed to bind address. Please try again.")
                .await?;
        }
    }

    Ok(())
}

async fn handle_unknown_command(bot: Bot, msg: Message, _state: BotState) -> BotResult<()> {
    bot.send_message(
        msg.chat.id,
        "❓ Unknown command. Use /help to see available commands.",
    )
    .await?;
    Ok(())
}

// Callback handlers
async fn handle_generate_wallet_callback(
    bot: Bot,
    callback: CallbackQuery,
    state: BotState,
) -> BotResult<()> {
    bot.answer_callback_query(callback.id).await?;

    if let Some(msg) = callback.message {
        if let Some(regular_msg) = msg.regular_message() {
            handle_generate_wallet(bot, regular_msg.clone(), state).await?;
        }
    }

    Ok(())
}

async fn handle_balance_callback(
    bot: Bot,
    callback: CallbackQuery,
    state: BotState,
) -> BotResult<()> {
    bot.answer_callback_query(callback.id).await?;

    if let Some(msg) = callback.message {
        if let Some(regular_msg) = msg.regular_message() {
            handle_balance(bot, regular_msg.clone(), state).await?;
        }
    }

    Ok(())
}
