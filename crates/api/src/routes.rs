// routes.rs — Axum router configuration

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use std::time::Duration;

use crate::{handlers, state::AppState};

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health
        .route("/health", get(handlers::health))
        // Wallet
        .route("/wallet/balance/:address", get(handlers::get_balance))
        .route("/wallet/transactions/:address", get(handlers::get_wallet_transactions))
        .route("/wallet/qr/:address", get(handlers::get_qr_code))
        // Transactions
        .route("/transaction/estimate", post(handlers::estimate_transaction))
        .route("/transaction/send", post(handlers::send_transaction))
        .route("/transaction/:signature", get(handlers::get_transaction))
        // Price
        .route("/price/sol", get(handlers::get_sol_price))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(Duration::from_secs(30)))
        .with_state(state)
}
