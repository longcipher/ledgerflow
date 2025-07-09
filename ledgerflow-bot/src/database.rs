#![allow(unused)]

use eyre::Result;
use sqlx::{PgPool, Row, migrate};
use tracing::info;

use crate::models::{Order, User};

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations...");
        migrate!("./migrations").run(&self.pool).await?;
        info!("Database migrations completed successfully");
        Ok(())
    }

    pub async fn create_user(&self, user: &User) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO users (telegram_id, username, first_name, last_name, evm_address)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (telegram_id) DO UPDATE SET
                username = EXCLUDED.username,
                first_name = EXCLUDED.first_name,
                last_name = EXCLUDED.last_name,
                evm_address = COALESCE(EXCLUDED.evm_address, users.evm_address),
                updated_at = NOW()
            "#,
        )
        .bind(user.telegram_id)
        .bind(&user.username)
        .bind(&user.first_name)
        .bind(&user.last_name)
        .bind(&user.evm_address)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_user_by_telegram_id(&self, telegram_id: i64) -> Result<Option<User>> {
        let row = sqlx::query(
            "SELECT id, telegram_id, username, first_name, last_name, evm_address, created_at, updated_at FROM users WHERE telegram_id = $1"
        )
        .bind(telegram_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(User {
                id: row.get("id"),
                telegram_id: row.get("telegram_id"),
                username: row.get("username"),
                first_name: row.get("first_name"),
                last_name: row.get("last_name"),
                evm_address: row.get("evm_address"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn update_user_evm_address(&self, telegram_id: i64, evm_address: &str) -> Result<()> {
        sqlx::query("UPDATE users SET evm_address = $1, updated_at = NOW() WHERE telegram_id = $2")
            .bind(evm_address)
            .bind(telegram_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_user_orders(&self, telegram_id: i64) -> Result<Vec<Order>> {
        let rows = sqlx::query(
            r#"
            SELECT o.id, o.order_id, o.account_id, o.broker_id, o.amount, o.token_address, o.status, o.created_at, o.updated_at, o.transaction_hash
            FROM orders o
            JOIN users u ON o.account_id = u.telegram_id::text
            WHERE u.telegram_id = $1
            ORDER BY o.created_at DESC
            "#
        )
        .bind(telegram_id)
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            result.push(Order {
                id: row.get("id"),
                order_id: row.get("order_id"),
                account_id: row.get("account_id"),
                broker_id: row.get("broker_id"),
                amount: row.get("amount"),
                token_address: row.get("token_address"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                transaction_hash: row.get("transaction_hash"),
            });
        }

        Ok(result)
    }

    pub async fn get_user_balance(&self, telegram_id: i64) -> Result<String> {
        let row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(amount::decimal), 0)::text as balance
            FROM orders o
            JOIN users u ON o.account_id = u.telegram_id::text
            WHERE u.telegram_id = $1 AND o.status = 'completed'
            "#,
        )
        .bind(telegram_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row
            .get::<Option<String>, _>("balance")
            .unwrap_or_else(|| "0".to_string()))
    }
}
