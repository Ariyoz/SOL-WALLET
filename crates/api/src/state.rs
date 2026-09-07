// state.rs — shared application state injected into every Axum handler

use std::sync::Arc;
use solana_client::nonblocking::rpc_client::RpcClient;
use storage::Db;

#[derive(Clone)]
pub struct AppState {
    pub rpc_client: Arc<RpcClient>,
    pub db: Arc<Db>,
    pub cluster: Arc<String>,
    #[allow(dead_code)] // kept for diagnostics and future RPC switching
    pub rpc_url: Arc<String>,
}

impl AppState {
    pub fn new(rpc_client: RpcClient, db: Db, cluster: String, rpc_url: String) -> Self {
        AppState {
            rpc_client: Arc::new(rpc_client),
            db: Arc::new(db),
            cluster: Arc::new(cluster),
            rpc_url: Arc::new(rpc_url),
        }
    }
}
