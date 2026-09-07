// price.rs — SOL/USD price fetch via CoinGecko public API
//
// The USD value is display-only and clearly labelled as approximate.
// It is NEVER used for financial calculations — all arithmetic uses lamports.

use crate::error::WalletError;

/// Returns the current SOL/USD price, or `None` on any failure.
/// Failures are non-fatal — the app works fine without the USD estimate.
///
/// NOTE: wallet-core doesn't pull in reqwest to keep it lean.
/// The API crate calls this endpoint directly.
/// This stub is kept for symmetry and future expansion.
pub async fn fetch_sol_usd_price() -> Result<Option<f64>, WalletError> {
    // Price fetch is handled by the API crate (which already depends on reqwest).
    // wallet-core stays free of HTTP dependencies.
    Ok(None)
}
