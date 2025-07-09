use eyre::Result;
use sqlx::PgPool;

use crate::{
    error::AppError,
    models::{Account, Order, OrderStatus},
};

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn create_order(&self, order: &Order) -> Result<Order, AppError> {
        let result = sqlx::query_as::<_, Order>(
            r#"
            INSERT INTO orders (id, order_id, account_id, broker_id, amount, token_address, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, order_id, account_id, broker_id, amount, token_address, status, created_at, updated_at, transaction_hash
            "#,
        )
        .bind(order.id)
        .bind(&order.order_id)
        .bind(&order.account_id)
        .bind(&order.broker_id)
        .bind(&order.amount)
        .bind(&order.token_address)
        .bind(&order.status)
        .bind(order.created_at)
        .bind(order.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn get_order_by_id(&self, order_id: &str) -> Result<Option<Order>, AppError> {
        let result = sqlx::query_as::<_, Order>(
            r#"
            SELECT id, order_id, account_id, broker_id, amount, token_address, status, created_at, updated_at, transaction_hash
            FROM orders
            WHERE order_id = $1
            "#,
        )
        .bind(order_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn get_pending_orders_count(&self, account_id: &str) -> Result<i64, AppError> {
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM orders WHERE account_id = $1 AND status = 'pending'",
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn get_account_balance(&self, account_id: &str) -> Result<(String, i64), AppError> {
        // Get total balance
        let total_balance = sqlx::query_scalar::<_, String>(
            r#"
            SELECT COALESCE(SUM(CAST(amount AS NUMERIC)), 0)::text
            FROM orders 
            WHERE account_id = $1 AND status = 'completed'
            "#,
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?;

        // Get count
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM orders WHERE account_id = $1 AND status = 'completed'",
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?;

        Ok((total_balance, count))
    }

    pub async fn list_pending_orders(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<Order>, AppError> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let result = sqlx::query_as::<_, Order>(
            r#"
            SELECT id, order_id, account_id, broker_id, amount, token_address, status, created_at, updated_at, transaction_hash
            FROM orders
            WHERE status = 'pending'
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn update_order_status(
        &self,
        order_id: &str,
        status: OrderStatus,
        transaction_hash: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE orders SET status = $1, transaction_hash = $2, updated_at = NOW() WHERE order_id = $3",
        )
        .bind(status)
        .bind(transaction_hash)
        .bind(order_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_or_update_account(&self, account: &Account) -> Result<Account, AppError> {
        let result = sqlx::query_as::<_, Account>(
            r#"
            INSERT INTO accounts (id, account_id, email, telegram_id, evm_address, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (account_id) 
            DO UPDATE SET 
                email = COALESCE($3, accounts.email),
                telegram_id = COALESCE($4, accounts.telegram_id),
                evm_address = COALESCE($5, accounts.evm_address),
                updated_at = $7
            RETURNING id, account_id, email, telegram_id, evm_address, created_at, updated_at
            "#,
        )
        .bind(account.id)
        .bind(&account.account_id)
        .bind(&account.email)
        .bind(&account.telegram_id)
        .bind(&account.evm_address)
        .bind(account.created_at)
        .bind(account.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }
}
