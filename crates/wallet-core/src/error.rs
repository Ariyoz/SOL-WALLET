use thiserror::Error;

#[derive(Debug, Error)]
pub enum WalletError {
    // Wallet / key errors
    #[error("Failed to generate mnemonic: {0}")]
    MnemonicGeneration(String),

    #[error("Invalid mnemonic phrase: {0}")]
    InvalidMnemonic(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("Invalid base58 private key: {0}")]
    InvalidPrivateKey(String),

    // Address / validation errors
    #[error("Invalid Solana address: {0}")]
    InvalidAddress(String),

    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    // Balance errors
    #[error("Insufficient SOL — have {have_lamports} lamports, need {need_lamports} lamports (including fee)")]
    InsufficientFunds {
        have_lamports: u64,
        need_lamports: u64,
    },

    // RPC / network errors
    #[error("RPC request failed: {0}")]
    RpcError(String),

    #[error("Failed to fetch latest blockhash: {0}")]
    BlockhashFetch(String),

    #[error("Transaction submission failed: {0}")]
    TransactionSubmit(String),

    #[error("Transaction confirmation timeout")]
    ConfirmationTimeout,

    #[error("Blockhash expired — please retry")]
    BlockhashExpired,

    #[error("Network connection failed: {0}")]
    ConnectionFailed(String),

    // Transaction errors
    #[error("Transaction signing failed: {0}")]
    SigningFailed(String),

    #[error("Transaction serialization failed: {0}")]
    SerializationFailed(String),

    // Storage errors
    #[error("Storage error: {0}")]
    Storage(String),

    // Price errors
    #[error("Price fetch failed: {0}")]
    PriceFetch(String),
}

/// Convert a WalletError to a user-friendly message (no internal details).
impl WalletError {
    pub fn user_message(&self) -> String {
        match self {
            WalletError::InvalidAddress(_) => {
                "The recipient address is not a valid Solana address.".into()
            }
            WalletError::InvalidAmount(_) => {
                "The amount entered is not valid. Please enter a positive SOL amount.".into()
            }
            WalletError::InsufficientFunds { have_lamports, need_lamports } => {
                let have_sol = lamports_to_sol(*have_lamports);
                let need_sol = lamports_to_sol(*need_lamports);
                format!(
                    "Insufficient SOL\nYou have: {:.9} SOL\nRequired: {:.9} SOL\nPlease reduce the transfer amount or add more SOL.",
                    have_sol, need_sol
                )
            }
            WalletError::BlockhashExpired => {
                "The transaction expired. Please try again.".into()
            }
            WalletError::ConfirmationTimeout => {
                "Transaction confirmation timed out. Check the explorer to see if it was confirmed.".into()
            }
            WalletError::ConnectionFailed(_) | WalletError::RpcError(_) => {
                "Unable to connect to the Solana network. Please check your internet connection and try again.".into()
            }
            WalletError::InvalidMnemonic(_) => {
                "The recovery phrase is invalid. Please check every word and try again.".into()
            }
            _ => "An unexpected error occurred. Please try again.".into(),
        }
    }
}

fn lamports_to_sol(lamports: u64) -> f64 {
    lamports as f64 / 1_000_000_000.0
}
