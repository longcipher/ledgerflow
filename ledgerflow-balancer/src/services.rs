#![allow(unused)]
use chrono::Utc;

use crate::{
    database::Database,
    error::AppError,
    models::{Account, Balance, CreateOrderRequest, Order, OrderStatus},
    utils::{generate_order_id, get_next_order_id_num},
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
        // Check if account has too many pending orders
        let pending_count = self.db.get_pending_orders_count(request.account_id).await?;
        if pending_count >= self.max_pending_orders as i64 {
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

        // Create order
        let order = Order {
            id: 0, // This will be set by the database
            order_id,
            account_id: request.account_id,
            broker_id,
            amount: request.amount,
            token_address: request.token_address,
            chain_id: request.chain_id,
            status: OrderStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            transaction_hash: None,
        };

        self.db.create_order(&order).await
    }

    pub async fn get_order(&self, order_id: &str) -> Result<Order, AppError> {
        self.db
            .get_order_by_id(order_id)
            .await?
            .ok_or_else(|| AppError::OrderNotFound(order_id.to_string()))
    }

    pub async fn get_account_balance(&self, account_id: i64) -> Result<(String, i64), AppError> {
        self.db.get_account_balance(account_id).await
    }

    pub async fn list_pending_orders(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<Order>, AppError> {
        self.db.list_pending_orders(limit, offset).await
    }

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

    pub async fn create_or_update_account(
        &self,
        username: String,
        email: Option<String>,
        telegram_id: Option<i64>,
        evm_address: Option<String>,
    ) -> Result<Account, AppError> {
        let account = Account {
            id: 0, // This will be set by the database
            username,
            email,
            telegram_id,
            evm_address,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.db.create_or_update_account(&account).await
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
        let mut processed_count = 0;

        for order in deposited_orders {
            match self.process_single_order(&order).await {
                Ok(_) => {
                    processed_count += 1;
                    tracing::info!(
                        "Successfully processed deposited order: {}, amount: {}",
                        order.order_id,
                        order.amount
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to process deposited order {}: {}",
                        order.order_id,
                        e
                    );
                }
            }
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
