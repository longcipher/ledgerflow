use std::{collections::HashMap, sync::Arc};

use teloxide::{dispatching::UpdateHandler, prelude::*, types::*};
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::{
    config::Config,
    database::Database,
    error::{BotError, BotResult},
    models::{Account, CreateOrderRequest, UserSession, UserState},
    services::BalancerService,
    wallet::{self, execute_deposit, execute_withdraw},
};

pub type SessionManager = Arc<RwLock<HashMap<i64, UserSession>>>;

pub type BotState = std::sync::Arc<AppState>;

pub struct AppState {
    pub database: Database,
    #[allow(dead_code)]
    pub config: Config,
    #[allow(dead_code)]
    pub balancer: BalancerService,
    pub sessions: SessionManager,
}

pub async fn create_handler(
    _bot: Bot,
    database: Database,
    config: Config,
) -> BotResult<UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>>> {
    let balancer = BalancerService::new(&config);
    let sessions = Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));

    let state = std::sync::Arc::new(AppState {
        database,
        config,
        balancer,
        sessions,
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

    let user = match &msg.from {
        Some(user) => user,
        None => return Ok(()),
    };

    let _telegram_id = user.id.0 as i64;
    let chat_id = msg.chat.id;

    if let Some(text) = msg.text() {
        let result = match text {
            "/start" => handle_start(bot.clone(), msg.clone(), state.clone()).await,
            "/help" => handle_help(bot.clone(), msg.clone(), state.clone()).await,
            "/menu" => handle_menu(bot.clone(), msg.clone(), state.clone()).await,
            _ => handle_text_input(bot.clone(), msg.clone(), state.clone()).await,
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

async fn handle_text_input(bot: Bot, msg: Message, state: BotState) -> BotResult<()> {
    let user = msg
        .from
        .as_ref()
        .ok_or_else(|| BotError::Config("User not found".to_string()))?;
    let telegram_id = user.id.0 as i64;
    let text = msg
        .text()
        .ok_or_else(|| BotError::Config("No text in message".to_string()))?;

    let session = {
        let sessions = state.sessions.read().await;
        sessions.get(&telegram_id).cloned()
    };

    if let Some(session) = session {
        match session.state {
            UserState::AwaitingEmail => {
                handle_email_input(bot, msg.chat.id, state, text, telegram_id).await
            }
            UserState::AwaitingUsername(ref email) => {
                handle_username_input(bot, msg.chat.id, state, text, email.clone(), telegram_id)
                    .await
            }
            UserState::AwaitingDepositAmount => {
                handle_deposit_amount_input(bot, msg.chat.id, state, text, telegram_id).await
            }
            UserState::None => {
                // Unknown command
                bot.send_message(
                    msg.chat.id,
                    "Unknown command. Use /help to see available commands.",
                )
                .await?;
                Ok(())
            }
        }
    } else {
        // Unknown command
        bot.send_message(
            msg.chat.id,
            "Unknown command. Use /help to see available commands.",
        )
        .await?;
        Ok(())
    }
}

async fn handle_callback(
    bot: Bot,
    callback: CallbackQuery,
    state: BotState,
) -> Result<(), teloxide::RequestError> {
    let callback_id = callback.id.clone();

    if let Some(ref data) = callback.data {
        let result = match data.as_str() {
            "nav_projects" => handle_nav_projects(bot.clone(), callback, state.clone()).await,
            "nav_wallet" => handle_nav_wallet(bot.clone(), callback, state.clone()).await,
            "nav_account" => handle_nav_account(bot.clone(), callback, state.clone()).await,
            "nav_main" => handle_nav_main(bot.clone(), callback, state.clone()).await,
            "wallet_deposit" => handle_wallet_deposit(bot.clone(), callback, state.clone()).await,
            "wallet_withdraw" => handle_wallet_withdraw(bot.clone(), callback, state.clone()).await,
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

async fn handle_start(bot: Bot, msg: Message, state: BotState) -> BotResult<()> {
    let user = msg
        .from
        .as_ref()
        .ok_or_else(|| BotError::Config("User not found".to_string()))?;
    let telegram_id = user.id.0 as i64;

    // Check if user is already registered
    match state
        .database
        .get_account_by_telegram_id(telegram_id)
        .await?
    {
        Some(account) => {
            // Already registered
            bot.send_message(
                msg.chat.id,
                format!(
                    "Hello, you are already registered! Your account ID is: {}. Type /menu to start using.",
                    account.id
                ),
            )
            .await?;
        }
        None => {
            // New user, start registration process
            bot.send_message(
                msg.chat.id,
                "Welcome! Let's start the registration process. Please enter your email address:",
            )
            .await?;

            // Set user state
            {
                let mut sessions = state.sessions.write().await;
                sessions.insert(
                    telegram_id,
                    UserSession {
                        state: UserState::AwaitingEmail,
                        temp_email: None,
                    },
                );
            }
        }
    }

    Ok(())
}

async fn handle_help(bot: Bot, msg: Message, _state: BotState) -> BotResult<()> {
    let help_text = "Available commands:\n\n\
        /start - Register or view your account ID.\n\
        /menu - Open main operation menu (requires registration first).\n\
        /help - Show this help information.";

    bot.send_message(msg.chat.id, help_text).await?;
    Ok(())
}

async fn handle_menu(bot: Bot, msg: Message, state: BotState) -> BotResult<()> {
    let user = msg
        .from
        .as_ref()
        .ok_or_else(|| BotError::Config("User not found".to_string()))?;
    let telegram_id = user.id.0 as i64;

    // Check if user is already registered
    match state
        .database
        .get_account_by_telegram_id(telegram_id)
        .await?
    {
        Some(_account) => {
            // Already registered, show main menu
            let keyboard = InlineKeyboardMarkup::new(vec![
                vec![
                    InlineKeyboardButton::callback("Projects", "nav_projects"),
                    InlineKeyboardButton::callback("Wallet", "nav_wallet"),
                ],
                vec![InlineKeyboardButton::callback("Account", "nav_account")],
            ]);

            bot.send_message(
                msg.chat.id,
                "Welcome back! Please select the operation you want to perform:",
            )
            .reply_markup(keyboard)
            .await?;
        }
        None => {
            // Not registered
            bot.send_message(
                msg.chat.id,
                "You are not registered yet, please send /start to register.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn handle_email_input(
    bot: Bot,
    chat_id: ChatId,
    state: BotState,
    email: &str,
    telegram_id: i64,
) -> BotResult<()> {
    // Simple email validation
    if !email.contains('@') || !email.contains('.') {
        bot.send_message(chat_id, "Invalid email format, please enter again.")
            .await?;
        return Ok(());
    }

    // Update session state
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(
            telegram_id,
            UserSession {
                state: UserState::AwaitingUsername(email.to_string()),
                temp_email: Some(email.to_string()),
            },
        );
    }

    bot.send_message(chat_id, "Great! Now please enter your username:")
        .await?;

    Ok(())
}

async fn handle_username_input(
    bot: Bot,
    chat_id: ChatId,
    state: BotState,
    username: &str,
    email: String,
    telegram_id: i64,
) -> BotResult<()> {
    // Basic username validation
    if username.len() < 3 {
        bot.send_message(
            chat_id,
            "Username does not meet requirements or is already taken, please enter again.",
        )
        .await?;
        return Ok(());
    }

    // Generate wallet
    let wallet = wallet::generate_wallet().await?;

    // Create encrypted private key (simplified here, should use more secure encryption in production)
    let encrypted_pk = format!("encrypted_{}", wallet.private_key);

    // Create account
    let account = Account {
        id: 0, // Will be set by database
        username: username.to_string(),
        telegram_id,
        email: Some(email.clone()),
        evm_address: Some(wallet.address.clone()),
        encrypted_pk: Some(encrypted_pk),
        is_admin: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let account_id = state.database.create_account(&account).await?;

    // Clear session state
    {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&telegram_id);
    }

    // Send success message
    let success_message = format!(
        "Registration successful!\n\n\
        Your account information:\n\
        - Account ID: {}\n\
        - Username: {}\n\
        - Email: {}\n\
        - Your exclusive wallet address: {}\n\n\
        Type /menu to start using the service.",
        account_id, username, email, wallet.address
    );

    bot.send_message(chat_id, success_message).await?;

    Ok(())
}

async fn handle_deposit_amount_input(
    bot: Bot,
    chat_id: ChatId,
    state: BotState,
    amount: &str,
    telegram_id: i64,
) -> BotResult<()> {
    // Validate amount format
    if amount.parse::<u64>().is_err() {
        bot.send_message(
            chat_id,
            "Invalid amount format, please enter a valid number.",
        )
        .await?;
        return Ok(());
    }

    // Get account information
    let account = state
        .database
        .get_account_by_telegram_id(telegram_id)
        .await?
        .ok_or_else(|| BotError::Config("Account not found".to_string()))?;

    // Create order using BalancerService
    let create_order_request = CreateOrderRequest {
        account_id: account.id,
        amount: None,
        token_address: None,
        chain_id: None,
        broker_id: None,
    };

    let response = state.balancer.create_order(create_order_request).await?;

    // Clear session state
    {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&telegram_id);
    }

    let private_key = state
        .database
        .get_account_evm_pk_by_id(account.id)
        .await?
        .ok_or_else(|| BotError::Config("Account private key not found".to_string()))?;
    let rpc_url = state.config.blockchain.rpc_url.clone();
    let contract_address = state.config.blockchain.payment_vault_address.clone();
    let order_id = response.order_id.clone();
    let amount = amount
        .parse::<u64>()
        .map_err(|e| BotError::Config(format!("Invalid amount format: {e}")))?;

    execute_deposit(rpc_url, private_key, contract_address, order_id, amount).await?;

    bot.send_message(
        chat_id,
        format!("Deposit request submitted! Your order ID is {}. We will notify you after transaction confirmation.", response.order_id),
    )
    .await?;

    Ok(())
}

// Callback handlers
async fn handle_nav_projects(bot: Bot, callback: CallbackQuery, _state: BotState) -> BotResult<()> {
    bot.answer_callback_query(callback.id.clone()).await?;

    if let Some(message) = callback.message {
        let chat = message.chat();
        bot.edit_message_text(
            chat.id,
            message.id(),
            "This feature is under development, please stay tuned!",
        )
        .await?;
    }

    Ok(())
}

async fn handle_nav_account(bot: Bot, callback: CallbackQuery, state: BotState) -> BotResult<()> {
    bot.answer_callback_query(callback.id.clone()).await?;

    let user = callback.from;
    let telegram_id = user.id.0 as i64;

    if let Some(message) = callback.message {
        let chat = message.chat();
        match state
            .database
            .get_account_by_telegram_id(telegram_id)
            .await?
        {
            Some(account) => {
                let balance = state.database.get_balance(account.id).await?;

                let account_info = format!(
                    "Your account information:\n\
                    - Account ID: {}\n\
                    - Username: {}\n\
                    - Email: {}\n\
                    - Telegram ID: {}\n\
                    - Wallet Address: {}\n\
                    - Account Balance: {} USDC",
                    account.id,
                    account.username,
                    account.email.as_deref().unwrap_or("Not set"),
                    account.telegram_id,
                    account.evm_address.as_deref().unwrap_or("Not set"),
                    balance
                );

                let keyboard =
                    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
                        "<< Back", "nav_main",
                    )]]);

                bot.edit_message_text(chat.id, message.id(), account_info)
                    .reply_markup(keyboard)
                    .await?;
            }
            None => {
                bot.edit_message_text(chat.id, message.id(), "Account not found")
                    .await?;
            }
        }
    }

    Ok(())
}

async fn handle_nav_wallet(bot: Bot, callback: CallbackQuery, _state: BotState) -> BotResult<()> {
    bot.answer_callback_query(callback.id.clone()).await?;

    if let Some(message) = callback.message {
        let chat = message.chat();
        let keyboard = InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("💰 Deposit", "wallet_deposit"),
                InlineKeyboardButton::callback("💸 Withdraw", "wallet_withdraw"),
            ],
            vec![InlineKeyboardButton::callback("<< Back", "nav_main")],
        ]);

        bot.edit_message_text(chat.id, message.id(), "Wallet Operations:")
            .reply_markup(keyboard)
            .await?;
    }

    Ok(())
}

async fn handle_nav_main(bot: Bot, callback: CallbackQuery, _state: BotState) -> BotResult<()> {
    bot.answer_callback_query(callback.id.clone()).await?;

    if let Some(message) = callback.message {
        let chat = message.chat();
        let keyboard = InlineKeyboardMarkup::new(vec![
            vec![
                InlineKeyboardButton::callback("Projects", "nav_projects"),
                InlineKeyboardButton::callback("Wallet", "nav_wallet"),
            ],
            vec![InlineKeyboardButton::callback("Account", "nav_account")],
        ]);

        bot.edit_message_text(
            chat.id,
            message.id(),
            "Welcome back! Please select the operation you want to perform:",
        )
        .reply_markup(keyboard)
        .await?;
    }

    Ok(())
}

async fn handle_wallet_deposit(
    bot: Bot,
    callback: CallbackQuery,
    state: BotState,
) -> BotResult<()> {
    bot.answer_callback_query(callback.id.clone()).await?;

    let user = callback.from;
    let telegram_id = user.id.0 as i64;

    if let Some(message) = callback.message {
        let chat = message.chat();
        bot.edit_message_text(
            chat.id,
            message.id(),
            "Please enter the USDC amount you want to deposit(1000000 = 1 USDC):",
        )
        .await?;

        // Set user state
        {
            let mut sessions = state.sessions.write().await;
            sessions.insert(
                telegram_id,
                UserSession {
                    state: UserState::AwaitingDepositAmount,
                    temp_email: None,
                },
            );
        }
    }

    Ok(())
}

async fn handle_wallet_withdraw(
    bot: Bot,
    callback: CallbackQuery,
    state: BotState,
) -> BotResult<()> {
    bot.answer_callback_query(callback.id.clone()).await?;

    let user = callback.from;
    let telegram_id = user.id.0 as i64;

    if let Some(message) = callback.message {
        let chat = message.chat();
        // Check if user is an administrator
        match state
            .database
            .get_account_by_telegram_id(telegram_id)
            .await?
        {
            Some(account) if account.is_admin => {
                let private_key = state
                    .database
                    .get_account_evm_pk_by_id(account.id)
                    .await?
                    .ok_or_else(|| BotError::Config("Account private key not found".to_string()))?;
                let rpc_url = state.config.blockchain.rpc_url.clone();
                let contract_address = state.config.blockchain.payment_vault_address.clone();

                execute_withdraw(rpc_url, private_key, contract_address).await?;

                bot.edit_message_text(
                    chat.id,
                    message.id(),
                    "Withdrawal operation has been triggered. Please check progress in the backend or on blockchain explorer.",
                )
                .await?;
            }
            Some(_) => {
                bot.edit_message_text(
                    chat.id,
                    message.id(),
                    "Insufficient permissions, only administrators can perform this operation.",
                )
                .await?;
            }
            None => {
                bot.edit_message_text(chat.id, message.id(), "Account not found")
                    .await?;
            }
        }
    }

    Ok(())
}
