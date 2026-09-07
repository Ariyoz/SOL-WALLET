// fee.rs — live Solana network fee estimation
//
// We NEVER hard-code the fee. Every estimate calls getFeeForMessage on the
// cluster to get the current lamports_per_signature value.

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    message::Message,
    pubkey::Pubkey,
};
// Use the non-deprecated system instruction crate.
use solana_system_interface::instruction as system_instruction;

use crate::error::WalletError;

/// Fetch the actual current network fee for a SOL transfer, in lamports.
pub async fn estimate_transfer_fee(
    rpc_client: &RpcClient,
    from: &Pubkey,
    to: &Pubkey,
    lamports: u64,
) -> Result<u64, WalletError> {
    let blockhash = rpc_client
        .get_latest_blockhash()
        .await
        .map_err(|e| WalletError::BlockhashFetch(e.to_string()))?;

    let instruction = system_instruction::transfer(from, to, lamports);
    let message = Message::new_with_blockhash(&[instruction], Some(from), &blockhash);

    rpc_client
        .get_fee_for_message(&message)
        .await
        .map_err(|e| WalletError::RpcError(e.to_string()))
}

/// Fee breakdown shown to the user before they confirm.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FeeEstimate {
    /// Actual Solana network fee, in lamports (queried from cluster).
    pub network_fee_lamports: u64,
    /// Wallet service fee — always 0 in v1, shown explicitly for transparency.
    pub service_fee_lamports: u64,
    /// Total (network + service).
    pub total_fee_lamports: u64,
    /// Approximate USD value of the network fee (display only).
    pub network_fee_usd: Option<f64>,
}

impl FeeEstimate {
    pub fn new(network_fee_lamports: u64, sol_usd_price: Option<f64>) -> Self {
        let service_fee_lamports = 0u64;
        let total_fee_lamports = network_fee_lamports + service_fee_lamports;
        let network_fee_usd = sol_usd_price
            .map(|price| (network_fee_lamports as f64 / 1_000_000_000.0) * price);
        FeeEstimate {
            network_fee_lamports,
            service_fee_lamports,
            total_fee_lamports,
            network_fee_usd,
        }
    }

    pub fn network_fee_sol_display(&self) -> String {
        format!("{:.9}", self.network_fee_lamports as f64 / 1_000_000_000.0)
    }

    pub fn total_fee_sol_display(&self) -> String {
        format!("{:.9}", self.total_fee_lamports as f64 / 1_000_000_000.0)
    }
}

/// Verify the sender has enough lamports to cover transfer + fee.
pub fn validate_balance_for_transfer(
    balance_lamports: u64,
    transfer_lamports: u64,
    fee_lamports: u64,
) -> Result<(), WalletError> {
    let required = transfer_lamports
        .checked_add(fee_lamports)
        .ok_or_else(|| WalletError::InvalidAmount("Amount + fee overflow".into()))?;

    if balance_lamports < required {
        Err(WalletError::InsufficientFunds {
            have_lamports: balance_lamports,
            need_lamports: required,
        })
    } else {
        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_fee_is_zero() {
        let est = FeeEstimate::new(5_000, None);
        assert_eq!(est.service_fee_lamports, 0);
        assert_eq!(est.total_fee_lamports, est.network_fee_lamports);
    }

    #[test]
    fn usd_estimate_computed() {
        let est = FeeEstimate::new(5_000, Some(200.0));
        let usd = est.network_fee_usd.unwrap();
        // 5000 lamports = 0.000005 SOL; 0.000005 * $200 = $0.001
        assert!((usd - 0.001).abs() < 1e-9, "expected $0.001, got usd={usd}");
    }

    #[test]
    fn no_price_gives_no_usd() {
        assert!(FeeEstimate::new(5_000, None).network_fee_usd.is_none());
    }

    #[test]
    fn balance_sufficient() {
        assert!(validate_balance_for_transfer(2_000_000, 1_000_000, 5_000).is_ok());
    }

    #[test]
    fn balance_exact_passes() {
        assert!(validate_balance_for_transfer(1_005_000, 1_000_000, 5_000).is_ok());
    }

    #[test]
    fn balance_one_lamport_short_fails() {
        let r = validate_balance_for_transfer(1_004_999, 1_000_000, 5_000);
        assert!(matches!(
            r,
            Err(WalletError::InsufficientFunds {
                have_lamports: 1_004_999,
                need_lamports: 1_005_000
            })
        ));
    }

    #[test]
    fn zero_balance_fails() {
        assert!(matches!(
            validate_balance_for_transfer(0, 1_000, 5_000),
            Err(WalletError::InsufficientFunds { .. })
        ));
    }
}
