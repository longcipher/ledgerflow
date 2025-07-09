#![allow(unused)]
use chrono::Utc;
use uuid::Uuid;

use crate::{
    database::Database,
    error::AppError,
    models::{Account, CreateOrderRequest, Order, OrderStatus},
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
        let pending_count = self
            .db
            .get_pending_orders_count(&request.account_id)
            .await?;
        if pending_count >= self.max_pending_orders as i64 {
            return Err(AppError::TooManyPendingOrders(request.account_id));
        }

        // Generate unique order ID
        let broker_id = request
            .broker_id
            .unwrap_or_else(|| "ledgerflow-vault".to_string());
        let order_id_num = get_next_order_id_num(&request.account_id);
        let order_id = generate_order_id(&broker_id, &request.account_id, order_id_num);

        // Create order
        let order = Order {
            id: Uuid::new_v4(),
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

    pub async fn get_account_balance(&self, account_id: &str) -> Result<(String, i64), AppError> {
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
        account_id: String,
        email: Option<String>,
        telegram_id: Option<String>,
        evm_address: Option<String>,
    ) -> Result<Account, AppError> {
        let account = Account {
            id: Uuid::new_v4(),
            account_id,
            email,
            telegram_id,
            evm_address,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.db.create_or_update_account(&account).await
    }
}
