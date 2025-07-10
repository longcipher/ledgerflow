use eyre::Result;
use sqlx::PgPool;

use crate::{
    error::AppError,
    models::{Account, Balance, Order, OrderStatus},
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

    pub async fn get_next_order_id_num(&self) -> Result<u64, sqlx::Error> {
        let result: (i64,) = sqlx::query_as("SELECT nextval('orders_id_seq')")
            .fetch_one(&self.pool)
            .await?;

        Ok(result.0 as u64)
    }

    pub async fn create_order(&self, order: &Order) -> Result<Order, AppError> {
        let result = sqlx::query_as::<_, Order>(
            r#"
            INSERT INTO orders (order_id, account_id, broker_id, amount, token_address, chain_id, status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, order_id, account_id, broker_id, amount, token_address, chain_id, status, created_at, updated_at, transaction_hash
            "#,
        )
        .bind(&order.order_id)
        .bind(order.account_id)
        .bind(&order.broker_id)
        .bind(&order.amount)
        .bind(&order.token_address)
        .bind(order.chain_id)
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
            SELECT id, order_id, account_id, broker_id, amount, token_address, chain_id, status, created_at, updated_at, transaction_hash
            FROM orders
            WHERE order_id = $1
            "#,
        )
        .bind(order_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn get_pending_orders_count(&self, account_id: i64) -> Result<i64, AppError> {
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM orders WHERE account_id = $1 AND status = 'pending'",
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn get_completed_orders_count(&self, account_id: i64) -> Result<i64, AppError> {
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM orders WHERE account_id = $1 AND status = 'completed'",
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn get_account_balance(&self, account_id: i64) -> Result<(String, i64), AppError> {
        // Get balance from balances table
        let balance_record = self.get_account_balance_record(account_id).await?;

        // Get count of completed orders
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM orders WHERE account_id = $1 AND status = 'completed'",
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?;

        Ok((balance_record.balance, count))
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
            SELECT id, order_id, account_id, broker_id, amount, token_address, chain_id, status, created_at, updated_at, transaction_hash
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
            INSERT INTO accounts (username, email, telegram_id, evm_address, encrypted_pk, is_admin, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (username) 
            DO UPDATE SET 
                email = COALESCE($2, accounts.email),
                telegram_id = COALESCE($3, accounts.telegram_id),
                evm_address = COALESCE($4, accounts.evm_address),
                encrypted_pk = COALESCE($5, accounts.encrypted_pk),
                is_admin = COALESCE($6, accounts.is_admin),
                updated_at = $8
            RETURNING id, username, email, telegram_id, evm_address, encrypted_pk, is_admin, created_at, updated_at
            "#,
        )
        .bind(&account.username)
        .bind(&account.email)
        .bind(account.telegram_id)
        .bind(&account.evm_address)
        .bind(&account.encrypted_pk)
        .bind(account.is_admin)
        .bind(account.created_at)
        .bind(account.updated_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    /// Register a new account
    pub async fn register_account(&self, account: &Account) -> Result<Account, AppError> {
        // Check for unique constraints before insertion
        if self
            .get_account_by_username(&account.username)
            .await?
            .is_some()
        {
            return Err(AppError::InvalidInput(format!(
                "Username '{}' already exists",
                account.username
            )));
        }

        if let Some(email) = &account.email
            && self.get_account_by_email(email).await?.is_some()
        {
            return Err(AppError::InvalidInput(format!(
                "Email '{email}' already exists"
            )));
        }

        if let Some(telegram_id) = account.telegram_id
            && self
                .get_account_by_telegram_id(telegram_id)
                .await?
                .is_some()
        {
            return Err(AppError::InvalidInput(format!(
                "Telegram ID '{telegram_id}' already exists"
            )));
        }

        let result = sqlx::query_as::<_, Account>(
            r#"
            INSERT INTO accounts (username, email, telegram_id, evm_address, encrypted_pk, is_admin, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            RETURNING id, username, email, telegram_id, evm_address, encrypted_pk, is_admin, created_at, updated_at
            "#,
        )
        .bind(&account.username)
        .bind(&account.email)
        .bind(account.telegram_id)
        .bind(&account.evm_address)
        .bind(&account.encrypted_pk)
        .bind(account.is_admin)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get account by username
    pub async fn get_account_by_username(
        &self,
        username: &str,
    ) -> Result<Option<Account>, AppError> {
        let result = sqlx::query_as::<_, Account>(
            r#"
            SELECT id, username, email, telegram_id, evm_address, encrypted_pk, is_admin, created_at, updated_at
            FROM accounts
            WHERE username = $1
            "#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get account by email
    pub async fn get_account_by_email(&self, email: &str) -> Result<Option<Account>, AppError> {
        let result = sqlx::query_as::<_, Account>(
            r#"
            SELECT id, username, email, telegram_id, evm_address, encrypted_pk, is_admin, created_at, updated_at
            FROM accounts
            WHERE email = $1
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get account by telegram_id
    pub async fn get_account_by_telegram_id(
        &self,
        telegram_id: i64,
    ) -> Result<Option<Account>, AppError> {
        let result = sqlx::query_as::<_, Account>(
            r#"
            SELECT id, username, email, telegram_id, evm_address, encrypted_pk, is_admin, created_at, updated_at
            FROM accounts
            WHERE telegram_id = $1
            "#,
        )
        .bind(telegram_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get account by id
    pub async fn get_account_by_id(&self, id: i64) -> Result<Option<Account>, AppError> {
        let result = sqlx::query_as::<_, Account>(
            r#"
            SELECT id, username, email, telegram_id, evm_address, encrypted_pk, is_admin, created_at, updated_at
            FROM accounts
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get all orders with deposited status
    pub async fn get_deposited_orders(&self) -> Result<Vec<Order>, AppError> {
        let result = sqlx::query_as::<_, Order>(
            r#"
            SELECT id, order_id, account_id, broker_id, amount, token_address, chain_id, status, created_at, updated_at, transaction_hash
            FROM orders
            WHERE status = 'deposited'
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }

    /// Begin a database transaction
    pub async fn begin_transaction(
        &self,
    ) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, AppError> {
        let tx = self.pool.begin().await?;
        Ok(tx)
    }

    /// Add amount to user's balance (with transaction support)
    pub async fn add_to_balance(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        account_id: i64,
        amount: &str,
    ) -> Result<(), AppError> {
        // Insert or update balance record
        sqlx::query(
            r#"
            INSERT INTO balances (account_id, balance, created_at, updated_at)
            VALUES ($1, $2, NOW(), NOW())
            ON CONFLICT (account_id)
            DO UPDATE SET 
                balance = (CAST(balances.balance AS NUMERIC) + CAST($2 AS NUMERIC))::text,
                updated_at = NOW()
            "#,
        )
        .bind(account_id)
        .bind(amount)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    /// Update order status within a transaction
    pub async fn update_order_status_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
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
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    /// Get account balance record
    pub async fn get_account_balance_record(&self, account_id: i64) -> Result<Balance, AppError> {
        // Try to get existing balance record
        let balance = sqlx::query_as::<_, Balance>(
            r#"
            SELECT id, account_id, balance, created_at, updated_at
            FROM balances
            WHERE account_id = $1
            "#,
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?;

        // If no balance record exists, create one with 0 balance
        if let Some(balance) = balance {
            Ok(balance)
        } else {
            let new_balance = sqlx::query_as::<_, Balance>(
                r#"
                INSERT INTO balances (account_id, balance, created_at, updated_at)
                VALUES ($1, '0', NOW(), NOW())
                RETURNING id, account_id, balance, created_at, updated_at
                "#,
            )
            .bind(account_id)
            .fetch_one(&self.pool)
            .await?;

            Ok(new_balance)
        }
    }
}
