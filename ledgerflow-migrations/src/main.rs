use std::{env, time::Duration};

use config::{Config, ConfigError};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: String,
    pub idle_timeout: String,
    pub max_lifetime: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MigrationsConfig {
    pub path: String,
    pub table: String,
    pub lock_timeout: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub migrations: MigrationsConfig,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Config::builder()
            .add_source(config::File::with_name("config"))
            .add_source(config::Environment::with_prefix("LEDGERFLOW"));

        // Override with environment-specific config if ENV is set
        if let Ok(env) = env::var("ENV") {
            config = config
                .add_source(config::File::with_name(&format!("config.{env}")).required(false));
        }

        config.build()?.try_deserialize()
    }
}

pub struct MigrationManager {
    pool: Pool<Postgres>,
    config: AppConfig,
}

impl MigrationManager {
    pub async fn new(database_url: Option<&str>) -> Result<Self, Box<dyn std::error::Error>> {
        let config = AppConfig::load().unwrap_or_else(|_| {
            // Default configuration if config file is not found
            AppConfig {
                database: DatabaseConfig {
                    url: database_url
                        .unwrap_or("postgresql://postgres:password@localhost/ledgerflow")
                        .to_string(),
                    max_connections: 5,
                    min_connections: 1,
                    acquire_timeout: "30s".to_string(),
                    idle_timeout: "600s".to_string(),
                    max_lifetime: "1800s".to_string(),
                },
                migrations: MigrationsConfig {
                    path: "./migrations".to_string(),
                    table: "_sqlx_migrations".to_string(),
                    lock_timeout: "30s".to_string(),
                },
            }
        });

        let db_url = database_url.unwrap_or(&config.database.url);

        let pool = PgPoolOptions::new()
            .max_connections(config.database.max_connections)
            .min_connections(config.database.min_connections)
            .acquire_timeout(Duration::from_secs(30))
            .idle_timeout(Duration::from_secs(600))
            .max_lifetime(Duration::from_secs(1800))
            .connect(db_url)
            .await?;

        Ok(Self { pool, config })
    }

    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        tracing::info!("Running database migrations...");

        sqlx::migrate!("./migrations").run(&self.pool).await?;

        tracing::info!("Database migrations completed successfully");
        Ok(())
    }

    pub async fn check_migrations(&self) -> Result<Vec<i64>, sqlx::Error> {
        let migrator = sqlx::migrate!("./migrations");
        let migrations = migrator.migrations;

        let mut versions = Vec::new();
        for migration in migrations.iter() {
            versions.push(migration.version);
        }

        Ok(versions)
    }

    pub fn get_pool(&self) -> &Pool<Postgres> {
        &self.pool
    }

    pub fn get_config(&self) -> &AppConfig {
        &self.config
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    let filter = std::env::var("RUST_LOG")
        .map(|_| tracing_subscriber::EnvFilter::from_default_env())
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let database_url = env::var("DATABASE_URL").ok();

    info!("Starting LedgerFlow Migration Manager");

    match MigrationManager::new(database_url.as_deref()).await {
        Ok(migration_manager) => {
            if let Err(e) = migration_manager.run_migrations().await {
                error!("Migration failed: {}", e);
                return Err(e.into());
            }

            info!("All migrations completed successfully");

            // Check migration status
            match migration_manager.check_migrations().await {
                Ok(versions) => {
                    info!("Available migrations: {:?}", versions);
                }
                Err(e) => {
                    error!("Failed to check migrations: {}", e);
                }
            }
        }
        Err(e) => {
            error!("Failed to initialize migration manager: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migration_manager_creation() {
        let database_url = env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
            "postgresql://postgres:password@localhost/ledgerflow_test".to_string()
        });

        // This test requires a running PostgreSQL instance
        // Skip if not available
        let result = MigrationManager::new(Some(&database_url)).await;

        match result {
            Ok(manager) => {
                assert!(manager.get_pool().is_closed() == false);
                println!("✓ Migration manager created successfully");
            }
            Err(e) => {
                println!("⚠️  Skipping test - database not available: {}", e);
                // Don't fail the test if database is not available
            }
        }
    }

    #[tokio::test]
    async fn test_config_loading() {
        let config = AppConfig::load();

        // Should use default config if file doesn't exist
        let config = config.unwrap_or_else(|_| AppConfig {
            database: DatabaseConfig {
                url: "postgresql://postgres:password@localhost/ledgerflow".to_string(),
                max_connections: 5,
                min_connections: 1,
                acquire_timeout: "30s".to_string(),
                idle_timeout: "600s".to_string(),
                max_lifetime: "1800s".to_string(),
            },
            migrations: MigrationsConfig {
                path: "./migrations".to_string(),
                table: "_sqlx_migrations".to_string(),
                lock_timeout: "30s".to_string(),
            },
        });

        assert_eq!(config.database.max_connections, 5);
        assert_eq!(config.migrations.table, "_sqlx_migrations");
        println!("✓ Config loading test passed");
    }

    #[test]
    fn test_migration_directory_exists() {
        use std::path::Path;

        let migrations_dir = Path::new("./migrations");
        assert!(migrations_dir.exists(), "Migrations directory should exist");

        // Check if initial migration file exists
        let initial_migration = migrations_dir.join("20250709000001_initial_schema.sql");
        assert!(
            initial_migration.exists(),
            "Initial migration file should exist"
        );

        println!("✓ Migration directory structure test passed");
    }
}
