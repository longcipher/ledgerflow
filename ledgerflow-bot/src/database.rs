#![allow(unused)]
use eyre::Result;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    models::{Account, Order},
    wallet::decrypt_private_key,
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

    pub async fn get_account_evm_pk_by_id(&self, account_id: i64) -> Result<Option<String>> {
        let row = sqlx::query("SELECT encrypted_pk FROM accounts WHERE id = $1")
            .bind(account_id)
            .fetch_optional(&self.pool)
            .await?;

        let encrypted_pk = row.map(|r| r.get::<String, _>("encrypted_pk"));
        let decrypted_pk = encrypted_pk
            .as_ref()
            .map(|pk| decrypt_private_key(pk))
            .transpose()?;

        Ok(decrypted_pk)
    }

    pub async fn create_account(&self, account: &Account) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO accounts (username, telegram_id, email, evm_address, encrypted_pk, is_admin)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(&account.username)
        .bind(account.telegram_id)
        .bind(&account.email)
        .bind(&account.evm_address)
        .bind(&account.encrypted_pk)
        .bind(account.is_admin)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    pub async fn get_account_by_telegram_id(&self, telegram_id: i64) -> Result<Option<Account>> {
        let row = sqlx::query(
            "SELECT id, username, telegram_id, email, evm_address, encrypted_pk, is_admin, created_at, updated_at FROM accounts WHERE telegram_id = $1"
        )
        .bind(telegram_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(Account {
                id: row.get("id"),
                username: row.get("username"),
                telegram_id: row.get("telegram_id"),
                email: row.get("email"),
                evm_address: row.get("evm_address"),
                encrypted_pk: row.get("encrypted_pk"),
                is_admin: row.get("is_admin"),
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

    pub async fn create_order(&self, order: &Order) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO orders (order_id, account_id, broker_id, amount, token_address, chain_id, status, notified)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(&order.order_id)
        .bind(order.account_id)
        .bind(&order.broker_id)
        .bind(&order.amount)
        .bind(&order.token_address)
        .bind(order.chain_id)
        .bind(&order.status)
        .bind(order.notified)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get("id"))
    }

    pub async fn get_account_orders(&self, telegram_id: i64) -> Result<Vec<Order>> {
        let rows = sqlx::query(
            r#"
            SELECT o.id, o.order_id, o.account_id, o.broker_id, o.amount, o.token_address, o.chain_id, o.status, o.created_at, o.updated_at, o.transaction_hash, o.notified
            FROM orders o
            JOIN accounts a ON o.account_id = a.id
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
                chain_id: row.get("chain_id"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                transaction_hash: row.get("transaction_hash"),
                notified: row.get("notified"),
            });
        }

        Ok(result)
    }

    pub async fn get_balance(&self, account_id: i64) -> Result<String> {
        let row = sqlx::query("SELECT balance FROM balances WHERE account_id = $1")
            .bind(account_id)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            Ok(row.get("balance"))
        } else {
            Ok("0".to_string())
        }
    }

    // Get completed but unnotified orders
    pub async fn get_completed_unnotified_orders(&self) -> Result<Vec<(Order, i64)>> {
        let rows = sqlx::query(
            r#"
            SELECT o.id, o.order_id, o.account_id, o.broker_id, o.amount, o.token_address, o.chain_id, o.status, o.created_at, o.updated_at, o.transaction_hash, o.notified, a.telegram_id
            FROM orders o
            JOIN accounts a ON o.account_id = a.id
            WHERE o.status = 'completed' AND o.notified = false
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::new();
        for row in rows {
            let order = Order {
                id: row.get("id"),
                order_id: row.get("order_id"),
                account_id: row.get("account_id"),
                broker_id: row.get("broker_id"),
                amount: row.get("amount"),
                token_address: row.get("token_address"),
                chain_id: row.get("chain_id"),
                status: row.get("status"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                transaction_hash: row.get("transaction_hash"),
                notified: row.get("notified"),
            };
            let telegram_id: i64 = row.get("telegram_id");
            result.push((order, telegram_id));
        }

        Ok(result)
    }

    // Update order notification status
    pub async fn mark_order_as_notified(&self, order_id: i64) -> Result<()> {
        sqlx::query("UPDATE orders SET notified = true WHERE id = $1")
            .bind(order_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
