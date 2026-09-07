// models.rs — database row types
//
// Security: NO private keys, seed phrases, or any secret data is stored here.
// Only public wallet addresses and transaction metadata.

use serde::{Deserialize, Serialize};

/// A watched wallet address (public key only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletRow {
    pub id: i64,
    pub address: String,
    pub label: Option<String>,
    /// RFC-3339 timestamp string.
    pub created_at: String,
}

/// A cached transaction record for the history display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRow {
    pub id: i64,
    pub signature: String,
    pub wallet_address: String,
    pub counterparty_address: String,
    /// "sent" or "received"
    pub direction: String,
    /// Amount in lamports, stored as TEXT.
    pub amount_lamports: String,
    /// Fee in lamports, stored as TEXT.
    pub fee_lamports: String,
    pub status: String,
    /// RFC-3339 block time, nullable.
    pub block_time: Option<String>,
    pub slot: Option<i64>,
    pub created_at: String,
}
