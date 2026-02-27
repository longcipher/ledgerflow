use chrono::Utc;
use tracing::{error, info, warn};

use crate::{
    database::Database,
    error::AppError,
    models::{Account, Balance, CreateOrderRequest, Order, OrderStatus, RegisterAccountRequest},
    utils::generate_order_id,
};

pub struct OrderService {
    db: Database,
    max_pending_orders: u32,
}

impl OrderService {
    pub fn new(db: Database, max_pending_orders: u32) -> Self {
        Self {
            db,
            max_pending_orders,
        }
    }

    pub async fn create_order(&self, request: CreateOrderRequest) -> Result<Order, AppError> {
        let CreateOrderRequest {
            account_id,
            amount,
            token_address,
            chain_id,
            broker_id,
        } = request;

        info!(
            "Creating order for account {}: amount={}, token={}, chain_id={}",
            account_id, amount, token_address, chain_id
        );

        Self::validate_amount(&amount)?;
        let normalized_token_address =
            crate::utils::normalize_evm_address(&token_address).map_err(AppError::InvalidInput)?;
        if chain_id <= 0 {
            return Err(AppError::InvalidInput(
                "chain_id must be greater than 0".to_string(),
            ));
        }

        // Check if account has too many pending orders
        let pending_count = self.db.get_pending_orders_count(account_id).await?;
        if pending_count >= self.max_pending_orders as i64 {
            warn!(
                "Account {} has reached maximum pending orders limit: {}/{}",
                account_id, pending_count, self.max_pending_orders
            );
            return Err(AppError::TooManyPendingOrders(account_id.to_string()));
        }

        // Generate unique order ID
        let broker_id = broker_id.unwrap_or_else(|| "ledgerflow".to_string());
        let order_id_num = self.db.get_next_order_id_num().await?;
        let order_id = generate_order_id(&broker_id, account_id, order_id_num);

        info!(
            "Generated order ID: {}, order_id_num: {}, for account {}",
            order_id, order_id_num, account_id
        );

        // Create order
        let order = Order {
            id: order_id_num,
            order_id,
            account_id,
            broker_id,
            amount,
            token_address: normalized_token_address,
            chain_id,
            status: OrderStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            transaction_hash: None,
        };

        let created_order = self.db.create_order(&order).await?;
        info!("Order created successfully: {}", created_order.order_id);

        Ok(created_order)
    }

    fn validate_amount(amount: &str) -> Result<(), AppError> {
        let trimmed = amount.trim();
        if trimmed.is_empty() {
            return Err(AppError::InvalidInput(
                "amount must not be empty".to_string(),
            ));
        }

        if !trimmed.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(AppError::InvalidInput(
                "amount must be an unsigned integer string".to_string(),
            ));
        }

        if trimmed.chars().all(|ch| ch == '0') {
            return Err(AppError::InvalidInput(
                "amount must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    pub async fn get_order(&self, order_id: &str) -> Result<Order, AppError> {
        self.db
            .get_order_by_id(order_id)
            .await?
            .ok_or_else(|| AppError::OrderNotFound(order_id.to_string()))
    }

    pub async fn get_completed_orders(&self, account_id: i64) -> Result<i64, AppError> {
        self.db.get_completed_orders_count(account_id).await
    }

    pub async fn list_pending_orders(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<Order>, AppError> {
        self.db.list_pending_orders(limit, offset).await
    }

    #[allow(unused)]
    pub async fn update_order_status(
        &self,
        order_id: &str,
        status: OrderStatus,
        transaction_hash: Option<&str>,
    ) -> Result<(), AppError> {
        self.db
            .update_order_status(order_id, status, transaction_hash)
            .await
    }
}

pub struct AccountService {
    db: Database,
}

pub struct RegisterAccountResult {
    pub account: Account,
    pub api_token: String,
}

impl AccountService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Register a new account
    pub async fn register_account(
        &self,
        request: RegisterAccountRequest,
    ) -> Result<RegisterAccountResult, AppError> {
        info!("Registering new account: username={}", request.username);

        // Check if username already exists
        if let Some(_existing) = self.db.get_account_by_username(&request.username).await? {
            warn!("Username '{}' already exists", request.username);
            return Err(AppError::InvalidInput(format!(
                "Username {} already exists",
                request.username
            )));
        }

        let evm_address = crate::utils::normalize_evm_address(&request.evm_address)
            .map_err(AppError::InvalidInput)?;
        let api_token = crate::utils::generate_api_token();
        let api_token_hash = crate::utils::hash_api_token(&api_token);

        let account = Account {
            id: 0, // This will be set by the database
            username: request.username.clone(),
            email: Some(request.email.clone()),
            telegram_id: Some(request.telegram_id),
            evm_address: Some(evm_address),
            encrypted_pk: None,
            api_token_hash: Some(api_token_hash),
            is_admin: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let created_account = self.db.register_account(&account).await?;
        info!(
            "Account registered successfully: id={}, username={}",
            created_account.id, created_account.username
        );

        Ok(RegisterAccountResult {
            account: created_account,
            api_token,
        })
    }

    /// Get account by username
    pub async fn get_account_by_username(
        &self,
        username: &str,
    ) -> Result<Option<Account>, AppError> {
        self.db.get_account_by_username(username).await
    }

    /// Get account by email
    pub async fn get_account_by_email(&self, email: &str) -> Result<Option<Account>, AppError> {
        self.db.get_account_by_email(email).await
    }

    /// Get account by telegram_id
    pub async fn get_account_by_telegram_id(
        &self,
        telegram_id: i64,
    ) -> Result<Option<Account>, AppError> {
        self.db.get_account_by_telegram_id(telegram_id).await
    }

    /// Get account by id
    #[allow(unused)]
    pub async fn get_account_by_id(&self, id: i64) -> Result<Option<Account>, AppError> {
        self.db.get_account_by_id(id).await
    }
}

pub struct BalanceService {
    db: Database,
}

impl BalanceService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Process deposited orders and update balances
    pub async fn process_deposited_orders(&self) -> Result<u32, AppError> {
        let deposited_orders = self.db.get_deposited_orders().await?;
        let orders_count = deposited_orders.len();
        let mut processed_count = 0;

        if !deposited_orders.is_empty() {
            info!("Processing {} deposited orders", orders_count);
        }

        for order in deposited_orders {
            match self.process_single_order(&order).await {
                Ok(processed) => {
                    if processed {
                        processed_count += 1;
                        info!(
                            "✅ Successfully processed deposited order: {}, amount: {} for account {}",
                            order.order_id, order.amount, order.account_id
                        );
                    } else {
                        info!(
                            "⏭️ Skipped already-processed deposited order: {}",
                            order.order_id
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "❌ Failed to process deposited order {}: {}",
                        order.order_id, e
                    );
                }
            }
        }

        if processed_count > 0 {
            info!(
                "✅ Batch processing completed: {}/{} orders processed successfully",
                processed_count, orders_count
            );
        }

        Ok(processed_count)
    }

    /// Process a single deposited order
    async fn process_single_order(&self, order: &Order) -> Result<bool, AppError> {
        // Start a transaction to ensure atomicity
        let mut tx = self.db.begin_transaction().await?;

        let claimed = self
            .db
            .mark_order_completed_if_deposited(&mut tx, &order.order_id)
            .await?;
        if !claimed {
            tx.rollback().await?;
            return Ok(false);
        }

        if let Err(e) = self
            .db
            .add_to_balance(&mut tx, order.account_id, &order.amount)
            .await
        {
            tx.rollback().await?;
            return Err(e);
        }

        tx.commit().await?;
        Ok(true)
    }

    /// Get account balance
    pub async fn get_account_balance(&self, account_id: i64) -> Result<Balance, AppError> {
        self.db.get_account_balance_record(account_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::OrderService;
    use crate::error::AppError;

    #[test]
    fn validate_amount_accepts_positive_integer() {
        assert!(OrderService::validate_amount("1250000").is_ok());
    }

    #[test]
    fn validate_amount_rejects_decimal_values() {
        let error = OrderService::validate_amount("1.2500").expect_err("decimal must be rejected");
        assert!(matches!(error, AppError::InvalidInput(_)));
    }

    #[test]
    fn validate_amount_rejects_zero() {
        let error = OrderService::validate_amount("0").expect_err("zero must be rejected");
        assert!(matches!(error, AppError::InvalidInput(_)));
    }

    #[test]
    fn validate_amount_rejects_non_numeric_values() {
        let error =
            OrderService::validate_amount("abc").expect_err("non-numeric value must be rejected");
        assert!(matches!(error, AppError::InvalidInput(_)));
    }
}
