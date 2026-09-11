// state.rs — shared application state injected into every Axum handler

use std::sync::Arc;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signer::keypair::Keypair;
use storage::Db;

#[derive(Clone)]
pub struct AppState {
    pub rpc_client: Arc<RpcClient>,
    pub db: Arc<Db>,
    pub cluster: Arc<String>,
    #[allow(dead_code)]
    pub rpc_url: Arc<String>,
    /// USDC wallet fee in micro-units (6 decimals). 10000 = 0.01 USDC.
    pub usdc_fee_units: u64,
    /// Treasury wallet address that receives the USDC fee.
    pub usdc_fee_wallet: Option<Arc<String>>,
    /// Optional server-side fee payer for ATA creation rent.
    pub fee_payer: Option<Arc<Keypair>>,
}

impl AppState {
    pub fn new(
        rpc_client: RpcClient,
        db: Db,
        cluster: String,
        rpc_url: String,
        usdc_fee_units: u64,
        usdc_fee_wallet: Option<String>,
        fee_payer: Option<Keypair>,
    ) -> Self {
        AppState {
            rpc_client: Arc::new(rpc_client),
            db: Arc::new(db),
            cluster: Arc::new(cluster),
            rpc_url: Arc::new(rpc_url),
            usdc_fee_units,
            usdc_fee_wallet: usdc_fee_wallet.map(Arc::new),
            fee_payer: fee_payer.map(Arc::new),
        }
    }
}
