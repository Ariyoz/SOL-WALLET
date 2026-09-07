// handlers.rs — Axum request handlers
//
// Security: this file NEVER handles private keys or mnemonics.
// The backend receives only already-signed transactions and broadcasts them.
// It does NOT sign anything.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use bincode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use solana_client::rpc_config::RpcTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::Signature,
    transaction::Transaction,
};
use solana_transaction_status_client_types::UiTransactionEncoding;
use std::str::FromStr;
use tracing::info;

use wallet_core::{
    balance::{get_balance_lamports, lamports_to_sol, parse_sol_to_lamports},
    fee::{estimate_transfer_fee, validate_balance_for_transfer, FeeEstimate},
    wallet::validate_address,
};

use crate::{error::ApiError, state::AppState};

// ─────────────────────────────────────────────────────────────────────────────
//  GET /health
// ─────────────────────────────────────────────────────────────────────────────

pub async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "solana-low-fee-wallet-api",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
//  GET /wallet/balance/:address
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct BalanceResponse {
    pub address: String,
    pub balance_lamports: u64,
    pub balance_sol: f64,
    /// Approximate USD value — display only, never used in calculations.
    pub balance_usd: Option<f64>,
}

pub async fn get_balance(
    State(state): State<AppState>,
    Path(address): Path<String>,
) -> Result<Json<BalanceResponse>, ApiError> {
    let pubkey = validate_address(&address)?;
    let balance_lamports = get_balance_lamports(&state.rpc_client, &pubkey).await?;
    let balance_sol = lamports_to_sol(balance_lamports);
    let sol_price = fetch_sol_price().await;
    let balance_usd = sol_price.map(|p| balance_sol * p);

    Ok(Json(BalanceResponse { address, balance_lamports, balance_sol, balance_usd }))
}

// ─────────────────────────────────────────────────────────────────────────────
//  POST /transaction/estimate
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EstimateRequest {
    pub from_address: String,
    pub to_address: String,
    /// SOL amount as decimal string, e.g. "0.001"
    pub amount_sol: String,
}

#[derive(Serialize)]
pub struct EstimateResponse {
    pub from_address: String,
    pub to_address: String,
    pub amount_lamports: u64,
    pub amount_sol: String,
    pub network_fee_lamports: u64,
    pub network_fee_sol: String,
    /// "≈ $0.00075 USD (estimate)" — approximate only.
    pub network_fee_usd: Option<String>,
    /// Always 0 in v1 — shown explicitly so there are no hidden fees.
    pub service_fee_lamports: u64,
    pub service_fee_sol: String,
    pub total_lamports: u64,
    pub total_sol: String,
    pub sender_balance_lamports: u64,
    pub sender_balance_sol: String,
    pub sufficient_funds: bool,
    pub insufficient_funds_message: Option<String>,
}

pub async fn estimate_transaction(
    State(state): State<AppState>,
    Json(body): Json<EstimateRequest>,
) -> Result<Json<EstimateResponse>, ApiError> {
    let from_pubkey = validate_address(&body.from_address)?;
    let to_pubkey   = validate_address(&body.to_address)?;
    let amount_lamports = parse_sol_to_lamports(&body.amount_sol)?;

    let balance_lamports =
        get_balance_lamports(&state.rpc_client, &from_pubkey).await?;
    let fee_lamports =
        estimate_transfer_fee(&state.rpc_client, &from_pubkey, &to_pubkey, amount_lamports)
            .await?;

    let sol_price    = fetch_sol_price().await;
    let fee_estimate = FeeEstimate::new(fee_lamports, sol_price);
    let total_lamports = amount_lamports
        .checked_add(fee_lamports)
        .ok_or_else(|| ApiError::BadRequest("Amount + fee overflow".into()))?;

    let (sufficient_funds, insufficient_funds_message) =
        match validate_balance_for_transfer(balance_lamports, amount_lamports, fee_lamports) {
            Ok(())  => (true, None),
            Err(e)  => (false, Some(e.user_message())),
        };

    let network_fee_usd = fee_estimate
        .network_fee_usd
        .map(|v| format!("≈ ${v:.5} USD (estimate)"));

    Ok(Json(EstimateResponse {
        from_address: body.from_address,
        to_address: body.to_address,
        amount_lamports,
        amount_sol: format!("{:.9}", lamports_to_sol(amount_lamports)),
        network_fee_lamports: fee_estimate.network_fee_lamports,
        network_fee_sol:      fee_estimate.network_fee_sol_display(),
        network_fee_usd,
        service_fee_lamports: 0,
        service_fee_sol:      "0.000000000".into(),
        total_lamports,
        total_sol:            format!("{:.9}", lamports_to_sol(total_lamports)),
        sender_balance_lamports: balance_lamports,
        sender_balance_sol:      format!("{:.9}", lamports_to_sol(balance_lamports)),
        sufficient_funds,
        insufficient_funds_message,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
//  POST /transaction/send
// ─────────────────────────────────────────────────────────────────────────────
//
// The client signs locally, then POSTs the base64-encoded signed bytes here.
// We broadcast verbatim — never modify, never re-sign.

#[derive(Deserialize)]
pub struct SendRequest {
    /// Base64-encoded, fully-signed Solana transaction (bincode).
    pub signed_transaction_base64: String,
}

#[derive(Serialize)]
pub struct SendResponse {
    pub signature: String,
    pub explorer_url: String,
}

pub async fn send_transaction(
    State(state): State<AppState>,
    Json(body): Json<SendRequest>,
) -> Result<Json<SendResponse>, ApiError> {
    let tx_bytes = B64.decode(&body.signed_transaction_base64)
        .map_err(|e| ApiError::BadRequest(format!("Invalid base64: {e}")))?;

    let tx: Transaction = bincode::deserialize(&tx_bytes)
        .map_err(|e| ApiError::BadRequest(format!("Invalid transaction bytes: {e}")))?;

    let signature = state
        .rpc_client
        .send_transaction(&tx)
        .await
        .map_err(|e| ApiError::from(wallet_core::WalletError::TransactionSubmit(e.to_string())))?;

    info!("Broadcasted transaction: {}", signature);

    Ok(Json(SendResponse {
        explorer_url: build_explorer_url(&signature.to_string(), &state.cluster),
        signature: signature.to_string(),
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
//  GET /transaction/:signature
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_transaction(
    State(state): State<AppState>,
    Path(signature_str): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let sig = Signature::from_str(&signature_str)
        .map_err(|_| ApiError::BadRequest("Invalid transaction signature".into()))?;

    let config = RpcTransactionConfig {
        encoding:   Some(UiTransactionEncoding::JsonParsed),
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: Some(0),
    };

    let tx = state
        .rpc_client
        .get_transaction_with_config(&sig, config)
        .await
        .map_err(|e| ApiError::NotFound(format!("Transaction not found: {e}")))?;

    let fee_lamports = tx.transaction.meta.as_ref().map(|m| m.fee).unwrap_or(0);
    let failed = tx.transaction.meta
        .as_ref()
        .and_then(|m| m.err.as_ref())
        .is_some();

    Ok(Json(json!({
        "signature":    signature_str,
        "slot":         tx.slot,
        "block_time":   tx.block_time,
        "fee_lamports": fee_lamports,
        "fee_sol":      format!("{:.9}", lamports_to_sol(fee_lamports)),
        "status":       if failed { "failed" } else { "confirmed" },
        "explorer_url": build_explorer_url(&signature_str, &state.cluster),
    })))
}

// ─────────────────────────────────────────────────────────────────────────────
//  GET /wallet/transactions/:address
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PaginationParams {
    pub limit:  Option<i64>,
    pub offset: Option<i64>,
}

pub async fn get_wallet_transactions(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, ApiError> {
    validate_address(&address)?;

    let limit  = params.limit.unwrap_or(20).min(100);
    let offset = params.offset.unwrap_or(0);

    let txs = state
        .db
        .get_transactions(&address, limit, offset)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let items: Vec<Value> = txs.iter().map(|tx| {
        let amount: u64 = tx.amount_lamports.parse().unwrap_or(0);
        let fee:    u64 = tx.fee_lamports.parse().unwrap_or(0);
        json!({
            "signature":            tx.signature,
            "direction":            tx.direction,
            "counterparty_address": tx.counterparty_address,
            "amount_lamports":      amount,
            "amount_sol":           format!("{:.9}", lamports_to_sol(amount)),
            "fee_lamports":         fee,
            "fee_sol":              format!("{:.9}", lamports_to_sol(fee)),
            "status":               tx.status,
            "block_time":           tx.block_time,
            "explorer_url":         build_explorer_url(&tx.signature, &state.cluster),
        })
    }).collect();

    let count = items.len();
    Ok(Json(json!({
        "address":      address,
        "transactions": items,
        "count":        count,
        "limit":        limit,
        "offset":       offset,
    })))
}

// ─────────────────────────────────────────────────────────────────────────────
//  GET /price/sol
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_sol_price() -> Json<Value> {
    let price = fetch_sol_price().await;
    Json(json!({
        "currency": "USD",
        "price":    price,
        "note":     "Approximate market price. Display only — never used for fee calculations.",
        "source":   "CoinGecko public API",
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
//  GET /wallet/qr/:address
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_qr_code(
    Path(address): Path<String>,
) -> Result<Json<Value>, ApiError> {
    use image::{codecs::png::PngEncoder, ExtendedColorType, ImageEncoder, Luma};
    use qrcode::{EcLevel, QrCode};

    validate_address(&address)?;

    let uri = format!("solana:{address}");
    let code = QrCode::with_error_correction_level(uri.as_bytes(), EcLevel::M)
        .map_err(|e| ApiError::Internal(format!("QR generation failed: {e}")))?;

    let img = code.render::<Luma<u8>>().build();
    let mut png: Vec<u8> = Vec::new();
    PngEncoder::new(std::io::Cursor::new(&mut png))
        .write_image(img.as_raw(), img.width(), img.height(), ExtendedColorType::L8)
        .map_err(|e| ApiError::Internal(format!("PNG encode failed: {e}")))?;

    Ok(Json(json!({
        "address":            address,
        "uri":                uri,
        "qr_code_png_base64": B64.encode(&png),
        "note":               "QR encodes 'solana:<address>' (Solana Pay compatible).",
    })))
}

// ─────────────────────────────────────────────────────────────────────────────
//  Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn build_explorer_url(signature: &str, cluster: &str) -> String {
    match cluster {
        "mainnet-beta" => format!("https://explorer.solana.com/tx/{signature}"),
        other          => format!("https://explorer.solana.com/tx/{signature}?cluster={other}"),
    }
}

/// Fetch SOL/USD price from CoinGecko (non-fatal, display only).
async fn fetch_sol_price() -> Option<f64> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .ok()?;

    let resp: serde_json::Value = client
        .get("https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    resp["solana"]["usd"].as_f64()
}

// bincode is used in send_transaction via bincode::deserialize
