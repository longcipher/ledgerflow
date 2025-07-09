#![allow(unused)]
use eyre::Result;
use sqlx::{PgPool, Row};

use crate::models::{Account, Order};

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn create_account(&self, account: &Account) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO accounts (account_id, telegram_id, email, evm_address)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (telegram_id) DO UPDATE SET
                account_id = EXCLUDED.account_id,
                email = EXCLUDED.email,
                evm_address = COALESCE(EXCLUDED.evm_address, accounts.evm_address),
                updated_at = NOW()
            "#,
        )
        .bind(&account.account_id)
        .bind(account.telegram_id)
        .bind(&account.email)
        .bind(&account.evm_address)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_account_by_telegram_id(&self, telegram_id: i64) -> Result<Option<Account>> {
        let row = sqlx::query(
            "SELECT id, account_id, telegram_id, email, evm_address, created_at, updated_at FROM accounts WHERE telegram_id = $1"
        )
        .bind(telegram_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Account {
                id: row.get("id"),
                account_id: row.get("account_id"),
                telegram_id: row.get("telegram_id"),
                email: row.get("email"),
                evm_address: row.get("evm_address"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn update_account_evm_address(
        &self,
        telegram_id: i64,
        evm_address: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE accounts SET evm_address = $1, updated_at = NOW() WHERE telegram_id = $2",
        )
        .bind(evm_address)
        .bind(telegram_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_account_orders(&self, telegram_id: i64) -> Result<Vec<Order>> {
        let rows = sqlx::query(
            r#"
            SELECT o.id, o.order_id, o.account_id, o.broker_id, o.amount, o.token_address, o.status, o.created_at, o.updated_at, o.transaction_hash
            FROM orders o
            JOIN accounts a ON o.account_id = a.telegram_id::text
            WHERE a.telegram_id = $1
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

    pub async fn get_account_balance(&self, telegram_id: i64) -> Result<String> {
        let row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(amount::decimal), 0)::text as balance
            FROM orders o
            JOIN accounts a ON o.account_id = a.telegram_id::text
            WHERE a.telegram_id = $1 AND o.status = 'completed'
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
