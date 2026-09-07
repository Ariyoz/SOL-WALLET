// wallet-core — non-custodial Solana wallet library
//
// Security guarantees exported from this crate:
//  * Private keys never leave the process.
//  * Private keys are never serialised to disk or over the wire.
//  * All transaction signing is performed locally via `Wallet::sign_transaction`.
//  * Only *signed* transactions are passed to RPC methods.
//  * Lamports (u64) are used for all financial arithmetic.

pub mod balance;
pub mod error;
pub mod fee;
pub mod price;
pub mod transaction;
pub mod wallet;

pub use error::WalletError;
pub use wallet::Wallet;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

/// Build a non-blocking RPC client from a URL.
pub fn create_rpc_client(url: &str) -> RpcClient {
    RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed())
}

/// Standard Solana cluster URLs.
pub mod cluster {
    pub const DEVNET: &str = "https://api.devnet.solana.com";
    pub const TESTNET: &str = "https://api.testnet.solana.com";
    pub const MAINNET_BETA: &str = "https://api.mainnet-beta.solana.com";
}
