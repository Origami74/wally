//! Storage for Nostr Wallet Connect connections
//!
//! Persists NWC connections to a SQLite database so they survive app restarts.

use crate::nwc::{BudgetRenewalPeriod, ConnectionBudget, WalletConnection};
use directories::ProjectDirs;
use nostr_sdk::{Keys, PublicKey, SecretKey, Timestamp};
use rusqlite::{params, Connection, Row};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

/// Storage manager for NWC connections
pub struct NwcConnectionStorage {
    db_path: PathBuf,
}

impl NwcConnectionStorage {
    /// Create a new storage manager
    pub fn new() -> Result<Self, StorageError> {
        let project_dirs =
            ProjectDirs::from("com", "Tollgate", "TollgateApp").ok_or_else(|| {
                StorageError::Path("Unable to determine storage directory".to_string())
            })?;

        let base_dir = project_dirs.data_dir().to_path_buf();

        // Create directory if it doesn't exist
        if let Some(parent) = base_dir.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(&base_dir)?;

        let db_path = base_dir.join("nwc-connections.sqlite");

        let storage = Self { db_path };
        storage.init_database()?;

        Ok(storage)
    }

    /// Initialize the database schema
    fn init_database(&self) -> Result<(), StorageError> {
        let conn = Connection::open(&self.db_path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS connections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                connection_secret TEXT NOT NULL UNIQUE,
                connection_pubkey TEXT NOT NULL,
                app_pubkey TEXT,
                secret TEXT,
                renewal_period TEXT NOT NULL,
                renews_at INTEGER,
                total_budget_msats INTEGER NOT NULL,
                used_budget_msats INTEGER NOT NULL,
                name TEXT,
                created_at INTEGER NOT NULL
            )",
            [],
        )?;

        let _ = conn.execute("ALTER TABLE connections ADD COLUMN name TEXT", []);

        Ok(())
    }

    /// Save a connection to the database
    pub fn save_connection(&self, connection: &WalletConnection) -> Result<(), StorageError> {
        let conn = Connection::open(&self.db_path)?;

        let connection_secret = connection.keys.secret_key().to_secret_hex();
        let connection_pubkey = connection.keys.public_key().to_hex();
        let app_pubkey = connection.app_pubkey.map(|pk| pk.to_hex());
        let secret = connection.secret.clone();

        let renewal_period = match connection.budget.renewal_period {
            BudgetRenewalPeriod::Daily => "daily",
            BudgetRenewalPeriod::Weekly => "weekly",
            BudgetRenewalPeriod::Monthly => "monthly",
            BudgetRenewalPeriod::Yearly => "yearly",
            BudgetRenewalPeriod::Never => "never",
        };

        let renews_at = connection.budget.renews_at.map(|t| t.as_u64() as i64);
        let total_budget_msats = connection.budget.total_budget_msats as i64;
        let used_budget_msats = connection.budget.used_budget_msats as i64;
        let created_at = Timestamp::now().as_u64() as i64;
        let name = &connection.name;

        conn.execute(
            "INSERT OR REPLACE INTO connections 
             (connection_secret, connection_pubkey, app_pubkey, secret, 
              renewal_period, renews_at, total_budget_msats, used_budget_msats, name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                connection_secret,
                connection_pubkey,
                app_pubkey,
                secret,
                renewal_period,
                renews_at,
                total_budget_msats,
                used_budget_msats,
                name,
                created_at,
            ],
        )?;

        log::info!("Saved NWC connection to database: {}", connection_pubkey);
        Ok(())
    }

    /// Load all connections from the database
    pub fn load_connections(&self) -> Result<Vec<WalletConnection>, StorageError> {
        let conn = Connection::open(&self.db_path)?;

        let mut stmt = conn.prepare(
            "SELECT connection_secret, connection_pubkey, app_pubkey, secret,
                    renewal_period, renews_at, total_budget_msats, used_budget_msats, name
             FROM connections
             ORDER BY created_at DESC",
        )?;

        let connections = stmt.query_map([], |row| Self::row_to_connection(row))?;

        let mut result = Vec::new();
        for connection in connections {
            match connection {
                Ok(conn) => result.push(conn),
                Err(e) => log::warn!("Failed to load connection from database: {}", e),
            }
        }

        log::info!("Loaded {} NWC connections from database", result.len());
        Ok(result)
    }

    /// Convert a database row to a WalletConnection
    fn row_to_connection(row: &Row) -> rusqlite::Result<WalletConnection> {
        let connection_secret: String = row.get(0)?;
        let app_pubkey_str: Option<String> = row.get(2)?;
        let secret: Option<String> = row.get(3)?;
        let renewal_period_str: String = row.get(4)?;
        let renews_at: Option<i64> = row.get(5)?;
        let total_budget_msats: i64 = row.get(6)?;
        let used_budget_msats: i64 = row.get(7)?;
        let name: Option<String> = row.get(8).unwrap_or(None);

        // Parse connection secret key
        let connection_secret_key = SecretKey::from_str(&connection_secret)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let keys = Keys::new(connection_secret_key);
        let default_name = WalletConnection::default_name(&keys);

        // Parse app pubkey if present
        let app_pubkey = if let Some(app_pk_str) = app_pubkey_str {
            Some(
                PublicKey::from_str(&app_pk_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
            )
        } else {
            None
        };

        // Parse renewal period
        let renewal_period = match renewal_period_str.as_str() {
            "daily" => BudgetRenewalPeriod::Daily,
            "weekly" => BudgetRenewalPeriod::Weekly,
            "monthly" => BudgetRenewalPeriod::Monthly,
            "yearly" => BudgetRenewalPeriod::Yearly,
            "never" => BudgetRenewalPeriod::Never,
            _ => BudgetRenewalPeriod::Daily,
        };

        let budget = ConnectionBudget {
            renewal_period,
            renews_at: renews_at.map(|t| Timestamp::from(t as u64)),
            total_budget_msats: total_budget_msats as u64,
            used_budget_msats: used_budget_msats as u64,
        };

        Ok(WalletConnection {
            keys,
            budget,
            app_pubkey,
            secret,
            name: name.unwrap_or(default_name),
        })
    }

    /// Delete a connection from the database
    pub fn delete_connection(&self, connection_pubkey: &str) -> Result<(), StorageError> {
        let conn = Connection::open(&self.db_path)?;

        conn.execute(
            "DELETE FROM connections WHERE connection_pubkey = ?1",
            params![connection_pubkey],
        )?;

        log::info!(
            "Deleted NWC connection from database: {}",
            connection_pubkey
        );
        Ok(())
    }

    /// Update the budget for a connection
    pub fn update_budget(
        &self,
        connection_pubkey: &str,
        budget: &ConnectionBudget,
    ) -> Result<(), StorageError> {
        let conn = Connection::open(&self.db_path)?;

        let renews_at = budget.renews_at.map(|t| t.as_u64() as i64);
        let total_budget_msats = budget.total_budget_msats as i64;
        let used_budget_msats = budget.used_budget_msats as i64;

        conn.execute(
            "UPDATE connections 
             SET renews_at = ?1, total_budget_msats = ?2, used_budget_msats = ?3
             WHERE connection_pubkey = ?4",
            params![
                renews_at,
                total_budget_msats,
                used_budget_msats,
                connection_pubkey
            ],
        )?;

        log::info!("Updated budget for NWC connection: {}", connection_pubkey);
        Ok(())
    }

    /// Update the display name for a connection
    pub fn update_name(&self, connection_pubkey: &str, name: &str) -> Result<(), StorageError> {
        let conn = Connection::open(&self.db_path)?;

        conn.execute(
            "UPDATE connections SET name = ?2 WHERE connection_pubkey = ?1",
            params![connection_pubkey, name],
        )?;

        log::info!("Updated name for NWC connection: {}", connection_pubkey);
        Ok(())
    }
}

/// Storage error types
#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path error: {0}")]
    Path(String),
}
