// storage — SQLite/PostgreSQL persistence for wallet history
//
// Security:
//  * NEVER store private keys, mnemonics, or seed phrases.
//  * Only public addresses and transaction metadata are persisted.

pub mod models;

use anyhow::Result;
use chrono::Utc;
use models::{TransactionRow, WalletRow};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Pool, Row, Sqlite,
};
use std::str::FromStr;
use tracing::info;
use wallet_core::transaction::{TransactionRecord, TransactionStatus};

/// Embedded migration SQL — run once on startup.
const MIGRATION_001: &str = include_str!("../migrations/001_init.sql");

pub struct Db {
    pool: Pool<Sqlite>,
}

impl Db {
    /// Connect (or create) a SQLite database.
    /// Example URL: `sqlite:./wallet.db`
    pub async fn connect(database_url: &str) -> Result<Self> {
        // Strip the "sqlite:" prefix to get the file path.
        let file_path = database_url
            .strip_prefix("sqlite:")
            .unwrap_or(database_url);

        let opts = SqliteConnectOptions::from_str(database_url)
            .unwrap_or_else(|_| {
                SqliteConnectOptions::new().filename(file_path)
            })
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        info!("Connected to database: {}", database_url);
        Ok(Db { pool })
    }

    /// Apply schema migrations at startup (idempotent — uses IF NOT EXISTS).
    pub async fn migrate(&self) -> Result<()> {
        sqlx::raw_sql(MIGRATION_001).execute(&self.pool).await?;
        info!("Database schema ready");
        Ok(())
    }

    // ── Wallet management ─────────────────────────────────────────────────

    /// Register or update a watched wallet address (public key only).
    /// Private keys and mnemonics are NEVER passed here.
    pub async fn upsert_wallet(&self, address: &str, label: Option<&str>) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO wallets (address, label, created_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(address) DO UPDATE SET label = excluded.label",
        )
        .bind(address)
        .bind(label)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_wallets(&self) -> Result<Vec<WalletRow>> {
        let rows = sqlx::query(
            "SELECT id, address, label, created_at FROM wallets ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| WalletRow {
            id: r.get("id"),
            address: r.get("address"),
            label: r.get("label"),
            created_at: r.get("created_at"),
        })
        .collect();
        Ok(rows)
    }

    // ── Transaction history ───────────────────────────────────────────────

    /// Insert or update a transaction record.
    pub async fn upsert_transaction(
        &self,
        wallet_address: &str,
        record: &TransactionRecord,
    ) -> Result<()> {
        // Ensure the wallet address exists in the wallets table first
        // (required by the foreign key constraint).
        self.upsert_wallet(wallet_address, None).await?;

        let direction = if record.from_address == wallet_address {
            "sent"
        } else {
            "received"
        };
        let counterparty = if direction == "sent" {
            &record.to_address
        } else {
            &record.from_address
        };

        let amount_str  = record.amount_lamports.to_string();
        let fee_str     = record.fee_lamports.to_string();
        let status_str  = match record.status {
            TransactionStatus::Confirmed  => "confirmed",
            TransactionStatus::Finalized  => "finalized",
            TransactionStatus::Failed     => "failed",
            TransactionStatus::Pending    => "pending",
            TransactionStatus::Timeout    => "timeout",
        };
        let block_time = chrono::DateTime::from_timestamp(record.timestamp, 0)
            .map(|dt: chrono::DateTime<Utc>| dt.to_rfc3339());
        let slot = record.slot.map(|s| s as i64);
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO transactions
                 (signature, wallet_address, counterparty_address, direction,
                  amount_lamports, fee_lamports, status, block_time, slot, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(signature) DO UPDATE SET status = excluded.status",
        )
        .bind(&record.signature)
        .bind(wallet_address)
        .bind(counterparty)
        .bind(direction)
        .bind(&amount_str)
        .bind(&fee_str)
        .bind(&status_str)
        .bind(block_time)
        .bind(slot)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Fetch paginated transaction history for a wallet address.
    pub async fn get_transactions(
        &self,
        wallet_address: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TransactionRow>> {
        let rows = sqlx::query(
            "SELECT id, signature, wallet_address, counterparty_address, direction,
                    amount_lamports, fee_lamports, status, block_time, slot, created_at
             FROM transactions
             WHERE wallet_address = ?1
             ORDER BY created_at DESC
             LIMIT ?2 OFFSET ?3",
        )
        .bind(wallet_address)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| TransactionRow {
            id: r.get("id"),
            signature: r.get("signature"),
            wallet_address: r.get("wallet_address"),
            counterparty_address: r.get("counterparty_address"),
            direction: r.get("direction"),
            amount_lamports: r.get("amount_lamports"),
            fee_lamports: r.get("fee_lamports"),
            status: r.get("status"),
            block_time: r.get("block_time"),
            slot: r.get("slot"),
            created_at: r.get("created_at"),
        })
        .collect();

        Ok(rows)
    }
}
