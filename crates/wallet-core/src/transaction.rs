// transaction.rs — SOL transfer construction, signing, and submission
//
// Security: the Keypair never leaves this crate. Only signed transaction bytes
// reach the RPC node. No private key is logged, serialised, or returned.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;

use crate::{
    balance::{get_balance_lamports, lamports_to_sol},
    error::WalletError,
    fee::{estimate_transfer_fee, validate_balance_for_transfer, FeeEstimate},
    wallet::Wallet,
};

// ── Data types ───────────────────────────────────────────────────────────────

/// A completed transaction — stored in history and shown in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub signature: String,
    pub from_address: String,
    pub to_address: String,
    /// Amount transferred, in lamports. Never f64.
    pub amount_lamports: u64,
    /// Actual network fee paid, in lamports.
    pub fee_lamports: u64,
    pub status: TransactionStatus,
    /// Unix timestamp, UTC seconds.
    pub timestamp: i64,
    pub slot: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionStatus {
    Confirmed,
    Finalized,
    Failed,
    Pending,
    Timeout,
}

impl TransactionRecord {
    pub fn amount_sol(&self) -> f64 {
        lamports_to_sol(self.amount_lamports)
    }
    pub fn fee_sol(&self) -> f64 {
        lamports_to_sol(self.fee_lamports)
    }
    pub fn explorer_url(&self, cluster: &str) -> String {
        match cluster {
            "mainnet-beta" => {
                format!("https://explorer.solana.com/tx/{}", self.signature)
            }
            other => {
                format!("https://explorer.solana.com/tx/{}?cluster={}", self.signature, other)
            }
        }
    }
}

/// Summary shown to the user before they confirm a send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferPreview {
    pub recipient: String,
    pub amount_lamports: u64,
    pub fee_estimate: FeeEstimate,
    /// amount + fee — total leaving the wallet.
    pub total_lamports: u64,
    pub sender_balance_lamports: u64,
}

impl TransferPreview {
    pub fn amount_sol_display(&self) -> String {
        format!("{:.9}", lamports_to_sol(self.amount_lamports))
    }
    pub fn total_sol_display(&self) -> String {
        format!("{:.9}", lamports_to_sol(self.total_lamports))
    }
    pub fn balance_sol_display(&self) -> String {
        format!("{:.9}", lamports_to_sol(self.sender_balance_lamports))
    }
}

// ── Step 1: build preview ────────────────────────────────────────────────────

pub async fn build_transfer_preview(
    rpc_client: &RpcClient,
    from_wallet: &Wallet,
    to_address: &Pubkey,
    amount_lamports: u64,
    sol_usd_price: Option<f64>,
) -> Result<TransferPreview, WalletError> {
    let from_pubkey = from_wallet.pubkey();
    let balance = get_balance_lamports(rpc_client, &from_pubkey).await?;
    let fee_lamports =
        estimate_transfer_fee(rpc_client, &from_pubkey, to_address, amount_lamports).await?;

    validate_balance_for_transfer(balance, amount_lamports, fee_lamports)?;

    let total_lamports = amount_lamports
        .checked_add(fee_lamports)
        .ok_or_else(|| WalletError::InvalidAmount("Amount + fee overflow".into()))?;

    Ok(TransferPreview {
        recipient: to_address.to_string(),
        amount_lamports,
        fee_estimate: FeeEstimate::new(fee_lamports, sol_usd_price),
        total_lamports,
        sender_balance_lamports: balance,
    })
}

// ── Step 2: execute transfer ─────────────────────────────────────────────────

/// Build, sign (locally), broadcast, and confirm a SOL transfer.
pub async fn execute_transfer(
    rpc_client: &RpcClient,
    from_wallet: &Wallet,
    to_address: &Pubkey,
    amount_lamports: u64,
) -> Result<TransactionRecord, WalletError> {
    let from_pubkey = from_wallet.pubkey();

    // Re-validate right before sending in case balance changed.
    let balance = get_balance_lamports(rpc_client, &from_pubkey).await?;
    let fee_lamports =
        estimate_transfer_fee(rpc_client, &from_pubkey, to_address, amount_lamports).await?;
    validate_balance_for_transfer(balance, amount_lamports, fee_lamports)?;

    let (blockhash, _) = rpc_client
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
        .await
        .map_err(|e| WalletError::BlockhashFetch(e.to_string()))?;

    let ix = system_instruction::transfer(&from_pubkey, to_address, amount_lamports);
    let mut tx = Transaction::new_with_payer(&[ix], Some(&from_pubkey));

    // Sign locally — keypair stays inside Wallet, never serialised.
    tx.sign(&[from_wallet.keypair()], blockhash);

    let signature: Signature = rpc_client
        .send_transaction(&tx)
        .await
        .map_err(|e| WalletError::TransactionSubmit(e.to_string()))?;

    tracing::info!("Transaction sent: {}", signature);

    let (status, slot) = match confirm_transaction(rpc_client, &signature).await {
        Ok(s) => {
            tracing::info!("Confirmed at slot {}", s);
            (TransactionStatus::Confirmed, Some(s))
        }
        Err(WalletError::ConfirmationTimeout) => {
            tracing::warn!("Confirmation timeout: {}", signature);
            (TransactionStatus::Timeout, None)
        }
        Err(e) => {
            tracing::error!("Transaction failed: {}", e);
            (TransactionStatus::Failed, None)
        }
    };

    Ok(TransactionRecord {
        signature: signature.to_string(),
        from_address: from_pubkey.to_string(),
        to_address: to_address.to_string(),
        amount_lamports,
        fee_lamports,
        status,
        timestamp: Utc::now().timestamp(),
        slot,
    })
}

// ── Confirmation polling ─────────────────────────────────────────────────────

async fn confirm_transaction(
    rpc_client: &RpcClient,
    signature: &Signature,
) -> Result<u64, WalletError> {
    const MAX: u32 = 60;
    for attempt in 0..MAX {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        match rpc_client.get_signature_status(signature).await {
            Ok(Some(Ok(()))) => {
                return Ok(rpc_client.get_slot().await.unwrap_or(0));
            }
            Ok(Some(Err(e))) => {
                return Err(WalletError::TransactionSubmit(e.to_string()));
            }
            Ok(None) => {
                tracing::debug!("Awaiting confirmation {}/{}", attempt + 1, MAX);
            }
            Err(e) => {
                tracing::warn!("Poll error ({}): {}", attempt + 1, e);
            }
        }
    }
    Err(WalletError::ConfirmationTimeout)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fee::FeeEstimate;

    fn make_record() -> TransactionRecord {
        TransactionRecord {
            signature: "5abc123def".into(),
            from_address: "FromAddr".into(),
            to_address: "ToAddr".into(),
            amount_lamports: 1_000_000,
            fee_lamports: 5_000,
            status: TransactionStatus::Confirmed,
            timestamp: 0,
            slot: Some(42),
        }
    }

    #[test]
    fn explorer_url_devnet() {
        let url = make_record().explorer_url("devnet");
        assert!(url.contains("explorer.solana.com"));
        assert!(url.contains("5abc123def"));
        assert!(url.contains("cluster=devnet"));
    }

    #[test]
    fn explorer_url_mainnet_no_cluster_param() {
        let url = make_record().explorer_url("mainnet-beta");
        assert!(url.contains("5abc123def"));
        assert!(!url.contains("cluster="));
    }

    #[test]
    fn sol_display_values() {
        let r = make_record();
        assert!((r.amount_sol() - 0.001).abs() < 1e-12);
        assert!((r.fee_sol() - 0.000005).abs() < 1e-12);
    }

    #[test]
    fn transfer_preview_displays() {
        let preview = TransferPreview {
            recipient: "x".into(),
            amount_lamports: 1_000_000,
            fee_estimate: FeeEstimate::new(5_000, None),
            total_lamports: 1_005_000,
            sender_balance_lamports: 10_000_000,
        };
        assert_eq!(preview.amount_sol_display(),  "0.001000000");
        assert_eq!(preview.total_sol_display(),   "0.001005000");
        assert_eq!(preview.balance_sol_display(), "0.010000000");
    }
}
