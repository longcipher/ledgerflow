use std::time::Duration;
use teloxide::{Bot, prelude::*};
use tokio::time::sleep;
use tracing::{error, info};

use crate::{database::Database, error::BotResult};

pub struct NotificationService {
    bot: Bot,
    database: Database,
}

impl NotificationService {
    pub fn new(bot: Bot, database: Database) -> Self {
        Self { bot, database }
    }

    pub async fn start_notification_loop(&self) -> BotResult<()> {
        info!("Starting notification service");
        
        loop {
            if let Err(e) = self.check_and_notify_completed_orders().await {
                error!("Error in notification service: {}", e);
            }
            
            // Check every minute
            sleep(Duration::from_secs(60)).await;
        }
    }

    async fn check_and_notify_completed_orders(&self) -> BotResult<()> {
        let orders = self.database.get_completed_unnotified_orders().await?;
        
        for (order, telegram_id) in orders {
            let message = format!(
                "✅ Deposit successful! Your order {} with amount {} USDC has been credited to your account.",
                order.order_id, order.amount
            );
            
            match self.bot.send_message(teloxide::types::ChatId(telegram_id), message).await {
                Ok(_) => {
                    // Mark as notified
                    if let Err(e) = self.database.mark_order_as_notified(order.id).await {
                        error!("Failed to mark order {} as notified: {}", order.id, e);
                    }
                }
                Err(e) => {
                    error!("Failed to send notification to user {}: {}", telegram_id, e);
                }
            }
        }
        
        Ok(())
    }
}
