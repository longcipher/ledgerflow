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
        info!(
            "Creating order for account {}: amount={}, token={}, chain_id={}",
            request.account_id,
            request.amount.as_deref().unwrap_or("0"),
            request.token_address.as_deref().unwrap_or("0x0"),
            request.chain_id.unwrap_or(0)
        );

        // Check if account has too many pending orders
        let pending_count = self.db.get_pending_orders_count(request.account_id).await?;
        if pending_count >= self.max_pending_orders as i64 {
            warn!(
                "Account {} has reached maximum pending orders limit: {}/{}",
                request.account_id, pending_count, self.max_pending_orders
            );
            return Err(AppError::TooManyPendingOrders(
                request.account_id.to_string(),
            ));
        }

        // Generate unique order ID
        let broker_id = request
            .broker_id
            .unwrap_or_else(|| "ledgerflow".to_string());
        let order_id_num = self.db.get_next_order_id_num().await?;
        let order_id = generate_order_id(&broker_id, request.account_id, order_id_num);

        info!(
            "Generated order ID: {}, order_id_num: {}, for account {}",
            order_id, order_id_num, request.account_id
        );

        // Create order
        let order = Order {
            id: order_id_num as i64,
            order_id,
            account_id: request.account_id,
            broker_id,
            amount: request.amount.unwrap_or_else(|| "0".to_string()),
            token_address: request.token_address.unwrap_or_else(|| "0x0".to_string()),
            chain_id: request.chain_id.unwrap_or(0),
            status: OrderStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            transaction_hash: None,
        };

        let created_order = self.db.create_order(&order).await?;
        info!("Order created successfully: {}", created_order.order_id);

        Ok(created_order)
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

impl AccountService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Register a new account
    pub async fn register_account(
        &self,
        request: RegisterAccountRequest,
    ) -> Result<Account, AppError> {
        info!("Registering new account: username={}", request.username);

        // Check if username already exists
        if let Some(_existing) = self.db.get_account_by_username(&request.username).await? {
            warn!("Username '{}' already exists", request.username);
            return Err(AppError::NotFound(format!(
                "Username {} already exists",
                request.username
            )));
        }

        // Process EVM private key (now required)
        let (evm_address, encrypted_pk) = {
            // Generate address from private key
            let address = match crate::utils::generate_evm_address_from_pk(&request.evm_pk) {
                Ok(addr) => addr,
                Err(e) => {
                    error!("Failed to generate EVM address: {}", e);
                    return Err(AppError::InvalidInput(format!(
                        "Invalid EVM private key: {e}"
                    )));
                }
            };

            // Encrypt private key
            let encrypted = match crate::utils::encrypt_private_key(&request.evm_pk) {
                Ok(enc) => enc,
                Err(e) => {
                    error!("Failed to encrypt private key: {}", e);
                    return Err(AppError::Internal(format!(
                        "Failed to encrypt private key: {e}"
                    )));
                }
            };

            (Some(address), Some(encrypted))
        };

        let account = Account {
            id: 0, // This will be set by the database
            username: request.username.clone(),
            email: Some(request.email.clone()),
            telegram_id: Some(request.telegram_id),
            evm_address,
            encrypted_pk,
            is_admin: request.is_admin.unwrap_or(false),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let created_account = self.db.register_account(&account).await?;
        info!(
            "Account registered successfully: id={}, username={}",
            created_account.id, created_account.username
        );

        Ok(created_account)
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
                Ok(_) => {
                    processed_count += 1;
                    info!(
                        "✅ Successfully processed deposited order: {}, amount: {} for account {}",
                        order.order_id, order.amount, order.account_id
                    );
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
    async fn process_single_order(&self, order: &Order) -> Result<(), AppError> {
        // Start a transaction to ensure atomicity
        let mut tx = self.db.begin_transaction().await?;

        // Add amount to user's balance
        match self
            .db
            .add_to_balance(&mut tx, order.account_id, &order.amount)
            .await
        {
            Ok(_) => {
                // Update order status to completed
                self.db
                    .update_order_status_tx(&mut tx, &order.order_id, OrderStatus::Completed, None)
                    .await?;

                // Commit transaction
                tx.commit().await?;
                Ok(())
            }
            Err(e) => {
                // Rollback transaction on error
                tx.rollback().await?;
                Err(e)
            }
        }
    }

    /// Get account balance
    pub async fn get_account_balance(&self, account_id: i64) -> Result<Balance, AppError> {
        self.db.get_account_balance_record(account_id).await
    }
}
