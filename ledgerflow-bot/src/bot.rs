#![allow(unused)]

use std::io::Cursor;

use base64::{engine::general_purpose, Engine};
use image::Rgb;
use qrcode::QrCode;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

use crate::{error::BotResult, models::PaymentDetails};

pub fn create_main_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("💰 Check Balance", "check_balance"),
            InlineKeyboardButton::callback("🆕 Generate Wallet", "generate_wallet"),
        ],
        vec![
            InlineKeyboardButton::callback("💳 Create Payment", "create_payment"),
            InlineKeyboardButton::callback("📊 View Orders", "view_orders"),
        ],
    ])
}

pub fn create_payment_keyboard() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("💳 Quick Pay 10 USDC", "pay_10"),
            InlineKeyboardButton::callback("💳 Quick Pay 50 USDC", "pay_50"),
        ],
        vec![
            InlineKeyboardButton::callback("💳 Quick Pay 100 USDC", "pay_100"),
            InlineKeyboardButton::callback("⚡ Custom Amount", "pay_custom"),
        ],
        vec![InlineKeyboardButton::callback("🔙 Back", "back_to_main")],
    ])
}

pub fn generate_qr_code(data: &str) -> BotResult<String> {
    let qr = QrCode::new(data)?;
    let image = qr.render::<Rgb<u8>>().build();

    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);

    image::write_buffer_with_format(
        &mut cursor,
        &image,
        image.width(),
        image.height(),
        image::ColorType::Rgb8,
        image::ImageFormat::Png,
    )?;

    let base64_image = general_purpose::STANDARD.encode(&buffer);
    Ok(format!("data:image/png;base64,{}", base64_image))
}

pub fn format_payment_details(details: &PaymentDetails) -> String {
    format!(
        "💳 **Payment Details**\n\n\
        🆔 Order ID: `{}`\n\
        💰 Amount: {} {}\n\
        📍 Payment Address: `{}`\n\
        🔗 Network: {}\n\n\
        📋 **Instructions:**\n\
        1. Send exactly {} {} to the payment address\n\
        2. Include the Order ID in transaction data\n\
        3. Wait for confirmation\n\n\
        ⚠️ **Important:** Use the exact amount and include the Order ID!",
        details.order_id,
        details.amount,
        details.token_symbol,
        details.payment_address,
        details.chain_name,
        details.amount,
        details.token_symbol
    )
}

pub fn format_balance_text(balance: &str, account_id: &str) -> String {
    format!(
        "💰 **Your Balance**\n\n\
        Total: {} USDC\n\
        Account: {}\n\n\
        Use /pay <amount> to create a payment request\n\
        Example: /pay 10.5",
        balance, account_id
    )
}

pub fn format_wallet_info(address: Option<&str>) -> String {
    match address {
        Some(addr) => format!(
            "👛 **Your Wallet**\n\n\
            Address: `{}`\n\n\
            Use /bind <address> to change your address\n\
            Use /generate_wallet to create a new one",
            addr
        ),
        None => "👛 **No Wallet Address**\n\n\
            Use /bind <address> to bind your address\n\
            Or use /generate_wallet to create a new one"
            .to_string(),
    }
}

pub fn format_order_list(orders: &[crate::models::Order]) -> String {
    if orders.is_empty() {
        return "📋 **Your Orders**\n\n\
            No orders found.\n\n\
            Use /pay <amount> to create your first payment request."
            .to_string();
    }

    let mut text = "📋 **Your Orders**\n\n".to_string();

    for (_i, order) in orders.iter().enumerate().take(10) {
        let status_emoji = match order.status.as_str() {
            "pending" => "⏳",
            "completed" => "✅",
            "failed" => "❌",
            "cancelled" => "🚫",
            _ => "❓",
        };

        text.push_str(&format!(
            "{} {} USDC - {} {}\n\
            Order ID: `{}`\n\
            Created: {}\n\n",
            status_emoji,
            order.amount,
            order.status.to_uppercase(),
            if let Some(tx) = &order.transaction_hash {
                format!("(TX: `{}`)", &tx[..8])
            } else {
                "".to_string()
            },
            order.order_id,
            order.created_at.format("%Y-%m-%d %H:%M UTC")
        ));
    }

    if orders.len() > 10 {
        text.push_str(&format!("... and {} more orders", orders.len() - 10));
    }

    text
}

pub fn escape_markdown(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '_' | '*' | '[' | ']' | '(' | ')' | '~' | '`' | '>' | '#' | '+' | '-' | '=' | '|'
            | '{' | '}' | '.' | '!' => {
                format!("\\{}", c)
            }
            _ => c.to_string(),
        })
        .collect()
}
