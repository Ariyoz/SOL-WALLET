// balance.rs — SOL balance lookup via non-blocking RPC

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

use crate::error::WalletError;

/// Returns the balance of `address` in lamports.
pub async fn get_balance_lamports(
    rpc_client: &RpcClient,
    address: &Pubkey,
) -> Result<u64, WalletError> {
    rpc_client
        .get_balance(address)
        .await
        .map_err(|e| WalletError::RpcError(e.to_string()))
}

/// Returns the balance in SOL (for display only — never use for arithmetic).
pub async fn get_balance_sol(
    rpc_client: &RpcClient,
    address: &Pubkey,
) -> Result<f64, WalletError> {
    let lamports = get_balance_lamports(rpc_client, address).await?;
    Ok(lamports_to_sol(lamports))
}

/// Convert lamports (u64) to SOL (f64) for *display purposes only*.
/// All arithmetic must be done in lamports (u64).
pub fn lamports_to_sol(lamports: u64) -> f64 {
    lamports as f64 / 1_000_000_000.0
}

/// Convert a SOL display value to lamports, rounding down.
/// Returns an error if the amount is negative, zero, or would overflow.
pub fn sol_to_lamports(sol: f64) -> Result<u64, WalletError> {
    if sol <= 0.0 {
        return Err(WalletError::InvalidAmount(format!(
            "Amount must be positive, got {}",
            sol
        )));
    }
    if !sol.is_finite() {
        return Err(WalletError::InvalidAmount("Amount is not a finite number".into()));
    }
    let lamports_f = sol * 1_000_000_000.0;
    if lamports_f > u64::MAX as f64 {
        return Err(WalletError::InvalidAmount("Amount too large".into()));
    }
    Ok(lamports_f as u64)
}

/// Parse a user-entered SOL string into lamports.
/// Accepts strings like "0.001", "0.000005", "1.5".
pub fn parse_sol_to_lamports(sol_str: &str) -> Result<u64, WalletError> {
    let trimmed = sol_str.trim();
    if trimmed.is_empty() {
        return Err(WalletError::InvalidAmount("Amount is empty".into()));
    }

    // Parse as a decimal string to avoid floating-point representation issues.
    // We split on '.' and compute lamports directly.
    let parts: Vec<&str> = trimmed.splitn(2, '.').collect();
    let whole: u64 = parts[0]
        .parse()
        .map_err(|_| WalletError::InvalidAmount(format!("Cannot parse '{}'", sol_str)))?;

    let fraction_lamports: u64 = if parts.len() == 2 {
        let frac_str = parts[1];
        if frac_str.len() > 9 {
            return Err(WalletError::InvalidAmount(
                "Amount has more than 9 decimal places (maximum precision is 1 lamport = 0.000000001 SOL)".into(),
            ));
        }
        let padded = format!("{:0<9}", frac_str); // right-pad to 9 digits
        padded[..9]
            .parse()
            .map_err(|_| WalletError::InvalidAmount(format!("Cannot parse fraction '{}'", frac_str)))?
    } else {
        0
    };

    let total = whole
        .checked_mul(1_000_000_000)
        .and_then(|w| w.checked_add(fraction_lamports))
        .ok_or_else(|| WalletError::InvalidAmount("Amount overflow".into()))?;

    if total == 0 {
        return Err(WalletError::InvalidAmount("Amount must be greater than zero".into()));
    }

    Ok(total)
}

// ------------------------------------------------------------------ //
//  Tests                                                               //
// ------------------------------------------------------------------ //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lamports_to_sol() {
        assert_eq!(lamports_to_sol(1_000_000_000), 1.0);
        assert_eq!(lamports_to_sol(500_000_000), 0.5);
        assert_eq!(lamports_to_sol(5_000), 0.000005);
        assert_eq!(lamports_to_sol(0), 0.0);
    }

    #[test]
    fn test_parse_sol_to_lamports() {
        assert_eq!(parse_sol_to_lamports("1").unwrap(), 1_000_000_000);
        assert_eq!(parse_sol_to_lamports("0.001").unwrap(), 1_000_000);
        assert_eq!(parse_sol_to_lamports("0.0001").unwrap(), 100_000);
        assert_eq!(parse_sol_to_lamports("0.0005").unwrap(), 500_000);
        assert_eq!(parse_sol_to_lamports("0.000005").unwrap(), 5_000);
        assert_eq!(parse_sol_to_lamports("0.000000001").unwrap(), 1); // 1 lamport
        assert_eq!(parse_sol_to_lamports("1.5").unwrap(), 1_500_000_000);
    }

    #[test]
    fn test_parse_sol_zero() {
        assert!(parse_sol_to_lamports("0").is_err());
        assert!(parse_sol_to_lamports("0.000000000").is_err());
    }

    #[test]
    fn test_parse_sol_invalid() {
        assert!(parse_sol_to_lamports("abc").is_err());
        assert!(parse_sol_to_lamports("").is_err());
        assert!(parse_sol_to_lamports("-0.001").is_err()); // negative
    }

    #[test]
    fn test_parse_sol_too_many_decimals() {
        assert!(parse_sol_to_lamports("0.0000000001").is_err()); // 10 decimals
    }

    #[test]
    fn test_sol_to_lamports() {
        assert_eq!(sol_to_lamports(1.0).unwrap(), 1_000_000_000);
        assert_eq!(sol_to_lamports(0.001).unwrap(), 1_000_000);
        assert!(sol_to_lamports(0.0).is_err());
        assert!(sol_to_lamports(-1.0).is_err());
    }
}
