// main.rs — Solana Low-Fee Wallet API server
//
// Starts the Axum HTTP server.
// Private keys NEVER reach this process — only public addresses and
// already-signed transactions are handled here.

mod error;
mod handlers;
mod routes;
mod state;

use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use wallet_core::create_rpc_client;
use storage::Db;
use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present.
    let _ = dotenv();

    // Initialise structured logging.
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    // Read configuration from environment.
    let rpc_url = std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| wallet_core::cluster::DEVNET.to_string());

    let cluster = std::env::var("SOLANA_NETWORK")
        .unwrap_or_else(|_| "devnet".to_string());

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:./wallet.db".to_string());

    let host = std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    // Render injects $PORT — fall back to API_PORT, then 3000
    let port: u16 = std::env::var("PORT")
        .or_else(|_| std::env::var("API_PORT"))
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .context("PORT must be a number")?;

    info!("Cluster: {}", cluster);
    info!("RPC URL: {}", rpc_url);

    // USDC fee configuration
    let usdc_fee_units: u64 = std::env::var("USDC_FEE_UNITS")
        .unwrap_or_else(|_| "10000".to_string())
        .parse()
        .unwrap_or(10000);
    let usdc_fee_wallet = std::env::var("USDC_FEE_WALLET").ok()
        .filter(|s| !s.is_empty());

    // Optional server-side fee payer for ATA creation
    let fee_payer = std::env::var("FEE_PAYER_KEYPAIR_BASE58").ok()
        .filter(|s| !s.is_empty())
        .and_then(|b58| {
            use solana_sdk::bs58;
            bs58::decode(&b58).into_vec().ok()
                .and_then(|bytes| solana_sdk::signer::keypair::Keypair::from_bytes(&bytes).ok())
        });

    if usdc_fee_wallet.is_some() {
        info!("USDC fee: {} units (0.{:06} USDC) → {}",
            usdc_fee_units, usdc_fee_units,
            usdc_fee_wallet.as_deref().unwrap_or("none"));
    }
    if fee_payer.is_some() {
        info!("Fee payer: configured (will fund ATA creation)");
    } else {
        info!("Fee payer: not configured (sender pays ATA rent)");
    }

    // Set up RPC client.
    let rpc_client = create_rpc_client(&rpc_url);

    // Set up database.
    let db = Db::connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    db.migrate()
        .await
        .context("Failed to run database migrations")?;

    info!("Database ready");

    // Build application state.
    let state = AppState::new(rpc_client, db, cluster, rpc_url.clone(),
        usdc_fee_units, usdc_fee_wallet, fee_payer);

    // Build router.
    let app = routes::build_router(state);

    // Start server.
    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .context("Invalid bind address")?;

    info!("Starting Solana Low-Fee Wallet API on {}", addr);
    info!("API endpoints:");
    info!("  GET  /health");
    info!("  GET  /wallet/balance/:address");
    info!("  GET  /wallet/transactions/:address");
    info!("  GET  /wallet/qr/:address");
    info!("  POST /transaction/estimate");
    info!("  POST /transaction/send");
    info!("  GET  /transaction/:signature");
    info!("  GET  /price/sol");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
