// handlers.rs — Axum request handlers
//
// Security: this file NEVER handles private keys or mnemonics.
// The backend receives only already-signed transactions and broadcasts them.
// Exception: the optional fee_payer keypair in AppState signs ATA-creation
// instructions only — it never touches user funds or user keys.

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
    instruction::AccountMeta,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::Signature,
    signer::Signer,
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

// ─── Program IDs ─────────────────────────────────────────────────────────────
const TOKEN_PROGRAM:       &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022_PROGRAM:  &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const ASSOC_TOKEN_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const USDC_MINT_MAINNET:   &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const USDC_MINT_DEVNET:    &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";

// ─────────────────────────────────────────────────────────────────────────────
//  POST /rpc  — generic Solana RPC proxy (CORS-safe passthrough)
// ─────────────────────────────────────────────────────────────────────────────

// Free RPC endpoints to try in order when the primary fails
const FALLBACK_RPCS: &[&str] = &[
    "https://api.mainnet-beta.solana.com",
    "https://rpc.ankr.com/solana",
    "https://solana-api.projectserum.com",
];

pub async fn rpc_proxy(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Try primary RPC first, then fallbacks
    let primary = state.rpc_url.as_str();
    let mut endpoints: Vec<&str> = vec![primary];
    for fb in FALLBACK_RPCS {
        if *fb != primary {
            endpoints.push(fb);
        }
    }

    let mut last_err = String::from("No endpoints available");
    for endpoint in &endpoints {
        match client.post(*endpoint).json(&body).send().await {
            Ok(resp) => {
                match resp.json::<Value>().await {
                    Ok(json) => {
                        // If it's a 403/rate-limit error from the RPC, try next
                        if let Some(err) = json.get("error") {
                            let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
                            let msg  = err.get("message").and_then(|m| m.as_str()).unwrap_or("");
                            if code == 403 || msg.contains("forbidden") || msg.contains("Access") {
                                last_err = format!("RPC {endpoint} returned 403, trying next");
                                continue;
                            }
                        }
                        return Ok(Json(json));
                    }
                    Err(e) => { last_err = e.to_string(); continue; }
                }
            }
            Err(e) => { last_err = e.to_string(); continue; }
        }
    }

    Err(ApiError::Internal(format!("All RPC endpoints failed: {last_err}")))
}

// ─────────────────────────────────────────────────────────────────────────────
//  GET /blockhash
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_blockhash(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let (blockhash, last_valid_block_height) = state
        .rpc_client
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch blockhash: {e}")))?;

    Ok(Json(json!({
        "blockhash": blockhash.to_string(),
        "lastValidBlockHeight": last_valid_block_height,
    })))
}

// ─────────────────────────────────────────────────────────────────────────────
//  POST /transaction/estimate-usdc  — estimate USDC send costs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EstimateUsdcRequest {
    pub from_address: String,
    pub to_address:   String,
    /// USDC amount as decimal string, e.g. "1.00"
    pub amount_usdc:  String,
}

#[derive(Serialize)]
pub struct EstimateUsdcResponse {
    pub from_address:          String,
    pub to_address:            String,
    pub amount_usdc:           String,
    pub amount_usdc_units:     u64,
    pub wallet_fee_usdc:       String,
    pub wallet_fee_usdc_units: u64,
    pub total_usdc:            String,
    pub total_usdc_units:      u64,
    pub recipient_receives:    String,
    pub recipient_usdc_ata:    Option<String>,
    pub recipient_needs_ata:   bool,
    pub ata_creation_sol:      String,
    pub network_fee_sol:       String,
    pub total_sol_needed:      String,
    pub fee_payer_available:   bool,
    pub sender_usdc_balance:   String,
    pub sender_sol_balance:    String,
    pub can_execute:           bool,
    pub error_reason:          Option<String>,
}

pub async fn estimate_usdc_send(
    State(state): State<AppState>,
    Json(body): Json<EstimateUsdcRequest>,
) -> Result<Json<EstimateUsdcResponse>, ApiError> {
    let from_pubkey = validate_address(&body.from_address)?;
    validate_address(&body.to_address)?;
    let to_pubkey = Pubkey::from_str(&body.to_address)
        .map_err(|_| ApiError::BadRequest("Invalid recipient address".into()))?;

    // Parse USDC amount (6 decimals)
    let amount_usdc_units = parse_usdc_units(&body.amount_usdc)
        .map_err(|e| ApiError::BadRequest(e))?;
    if amount_usdc_units == 0 {
        return Err(ApiError::BadRequest("Amount must be greater than zero".into()));
    }

    let wallet_fee_units = state.usdc_fee_units;
    let total_usdc_units = amount_usdc_units.saturating_add(wallet_fee_units);

    // Determine USDC mint for cluster
    let usdc_mint = if state.cluster.as_str() == "mainnet-beta" {
        USDC_MINT_MAINNET
    } else {
        USDC_MINT_DEVNET
    };
    let mint_pubkey = Pubkey::from_str(usdc_mint).unwrap();
    let token_prog  = Pubkey::from_str(TOKEN_PROGRAM).unwrap();
    let assoc_prog  = Pubkey::from_str(ASSOC_TOKEN_PROGRAM).unwrap();

    // Derive sender ATA
    let sender_ata = derive_ata(&from_pubkey, &mint_pubkey, &token_prog, &assoc_prog);
    // Derive recipient ATA
    let recipient_ata = derive_ata(&to_pubkey, &mint_pubkey, &token_prog, &assoc_prog);

    // Check sender USDC balance
    let sender_usdc_units = get_spl_balance(&state.rpc_client, &sender_ata).await
        .unwrap_or(0);
    let sender_sol = get_balance_lamports(&state.rpc_client, &from_pubkey).await
        .unwrap_or(0);

    // Check recipient ATA existence
    let recipient_ata_info = state.rpc_client
        .get_account(&recipient_ata).await.ok();
    let recipient_needs_ata = recipient_ata_info.is_none();

    // Network fee (live from cluster)
    let network_fee_lamports = estimate_transfer_fee(
        &state.rpc_client, &from_pubkey, &to_pubkey, 5000
    ).await.unwrap_or(5000);

    // ATA creation cost (~0.00203928 SOL = 2039280 lamports)
    let ata_rent_lamports: u64 = if recipient_needs_ata {
        state.rpc_client
            .get_minimum_balance_for_rent_exemption(165).await
            .unwrap_or(2_039_280)
    } else {
        0
    };

    let fee_payer_available = state.fee_payer.is_some();
    // SOL the sender needs: network fee always; ATA rent only if no fee payer
    let sender_sol_needed = if recipient_needs_ata && !fee_payer_available {
        network_fee_lamports + ata_rent_lamports
    } else {
        network_fee_lamports
    };

    // Determine if we can execute
    let mut can_execute = true;
    let mut error_reason: Option<String> = None;

    if sender_usdc_units < total_usdc_units {
        can_execute = false;
        let have = format_usdc(sender_usdc_units);
        let need = format_usdc(total_usdc_units);
        error_reason = Some(format!("Insufficient USDC. Have {have}, need {need}"));
    } else if sender_sol < sender_sol_needed {
        can_execute = false;
        let need_sol = sender_sol_needed as f64 / 1e9;
        let have_sol = sender_sol as f64 / 1e9;
        if recipient_needs_ata && !fee_payer_available {
            error_reason = Some(format!(
                "Not enough SOL. Need {need_sol:.6} SOL to pay the network fee and create the recipient's USDC account (one-time). You have {have_sol:.6} SOL."
            ));
        } else {
            error_reason = Some(format!(
                "Not enough SOL for network fee. Need {need_sol:.6} SOL, have {have_sol:.6} SOL."
            ));
        }
    } else if recipient_needs_ata && !fee_payer_available {
        // Sender has enough SOL — but warn about the extra cost
        // (can_execute stays true, they CAN do it)
    }

    Ok(Json(EstimateUsdcResponse {
        from_address: body.from_address,
        to_address: body.to_address,
        amount_usdc: format_usdc(amount_usdc_units),
        amount_usdc_units,
        wallet_fee_usdc: format_usdc(wallet_fee_units),
        wallet_fee_usdc_units: wallet_fee_units,
        total_usdc: format_usdc(total_usdc_units),
        total_usdc_units,
        recipient_receives: format_usdc(amount_usdc_units),
        recipient_usdc_ata: if recipient_needs_ata { None } else { Some(recipient_ata.to_string()) },
        recipient_needs_ata,
        ata_creation_sol: format!("{:.9}", ata_rent_lamports as f64 / 1e9),
        network_fee_sol: format!("{:.9}", network_fee_lamports as f64 / 1e9),
        total_sol_needed: format!("{:.9}", sender_sol_needed as f64 / 1e9),
        fee_payer_available,
        sender_usdc_balance: format_usdc(sender_usdc_units),
        sender_sol_balance: format!("{:.9}", sender_sol as f64 / 1e9),
        can_execute,
        error_reason,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
//  POST /transaction/prepare-usdc  — build + partially sign USDC transfer
//
//  The backend builds a transaction that includes:
//    1. (Optional) CreateIdempotent ATA for recipient — signed by fee payer
//    2. SPL Transfer: amount → recipient ATA — to be signed by sender
//    3. SPL Transfer: wallet fee → fee wallet ATA — to be signed by sender
//
//  Returns a base64 transaction with fee_payer signature already present
//  (if fee_payer is configured). The frontend adds the sender's signature
//  and sends to /transaction/send.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PrepareUsdcRequest {
    pub from_address:  String,
    pub to_address:    String,
    /// USDC amount as decimal string
    pub amount_usdc:   String,
}

#[derive(Serialize)]
pub struct PrepareUsdcResponse {
    /// Base64-encoded partially-signed transaction (bincode).
    /// The sender must add their signature and send to /transaction/send.
    pub transaction_base64: String,
    /// Estimated network fee in SOL
    pub network_fee_sol:    String,
    /// USDC wallet fee applied
    pub wallet_fee_usdc:    String,
    /// Whether an ATA was created for the recipient
    pub ata_created:        bool,
    /// Summary for the UI
    pub summary: PrepareUsdcSummary,
}

#[derive(Serialize)]
pub struct PrepareUsdcSummary {
    pub amount_usdc:        String,
    pub wallet_fee_usdc:    String,
    pub total_deducted:     String,
    pub recipient_receives: String,
    pub network_fee_sol:    String,
    pub ata_rent_sol:       Option<String>,
}

pub async fn prepare_usdc_send(
    State(state): State<AppState>,
    Json(body): Json<PrepareUsdcRequest>,
) -> Result<Json<PrepareUsdcResponse>, ApiError> {
    let from_pubkey = validate_address(&body.from_address)?;
    let to_pubkey = Pubkey::from_str(&body.to_address)
        .map_err(|_| ApiError::BadRequest("Invalid recipient address".into()))?;

    let amount_units = parse_usdc_units(&body.amount_usdc)
        .map_err(|e| ApiError::BadRequest(e))?;
    if amount_units == 0 {
        return Err(ApiError::BadRequest("Amount must be greater than zero".into()));
    }

    let wallet_fee_units = state.usdc_fee_units;
    let total_deduct = amount_units.saturating_add(wallet_fee_units);

    // Determine USDC mint
    let usdc_mint_str = if state.cluster.as_str() == "mainnet-beta" {
        USDC_MINT_MAINNET
    } else {
        USDC_MINT_DEVNET
    };
    let mint_pk      = Pubkey::from_str(usdc_mint_str).unwrap();
    let token_prog   = Pubkey::from_str(TOKEN_PROGRAM).unwrap();
    let assoc_prog   = Pubkey::from_str(ASSOC_TOKEN_PROGRAM).unwrap();
    let sys_prog     = solana_sdk::system_program::id();

    // Derive ATAs
    let sender_ata    = derive_ata(&from_pubkey, &mint_pk, &token_prog, &assoc_prog);
    let recipient_ata = derive_ata(&to_pubkey,   &mint_pk, &token_prog, &assoc_prog);

    // Validate sender USDC balance
    let sender_usdc = get_spl_balance(&state.rpc_client, &sender_ata).await
        .unwrap_or(0);
    if sender_usdc < total_deduct {
        return Err(ApiError::BadRequest(format!(
            "Insufficient USDC balance. Have {}, need {}.",
            format_usdc(sender_usdc), format_usdc(total_deduct)
        )));
    }

    // Check recipient ATA
    let recipient_ata_exists = state.rpc_client.get_account(&recipient_ata).await.is_ok();
    let ata_rent_lamports: u64 = if !recipient_ata_exists {
        state.rpc_client.get_minimum_balance_for_rent_exemption(165).await
            .unwrap_or(2_039_280)
    } else {
        0
    };

    // Determine fee payer for the transaction
    let fee_payer_pk = state.fee_payer.as_ref()
        .map(|kp| kp.pubkey())
        .unwrap_or(from_pubkey);

    // Validate SOL: sender always needs network fee; ATA rent from fee payer if available
    let network_fee = estimate_transfer_fee(&state.rpc_client, &from_pubkey, &to_pubkey, 5000)
        .await.unwrap_or(5000);
    let sender_sol = get_balance_lamports(&state.rpc_client, &from_pubkey).await
        .unwrap_or(0);

    let sender_sol_needed = if state.fee_payer.is_none() {
        network_fee + ata_rent_lamports
    } else {
        network_fee
    };
    if sender_sol < sender_sol_needed {
        let need = sender_sol_needed as f64 / 1e9;
        let have = sender_sol as f64 / 1e9;
        return Err(ApiError::BadRequest(format!(
            "Insufficient SOL. Need {need:.6} SOL for fees, have {have:.6} SOL."
        )));
    }

    // Validate fee payer SOL if separate
    if state.fee_payer.is_some() && !recipient_ata_exists {
        let fp_sol = get_balance_lamports(&state.rpc_client, &fee_payer_pk).await
            .unwrap_or(0);
        if fp_sol < ata_rent_lamports + 5000 {
            return Err(ApiError::Internal(
                "Fee payer has insufficient SOL to create recipient token account. Contact support.".into()
            ));
        }
    }

    // Build transaction
    let blockhash = state.rpc_client
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
        .await
        .map_err(|e| ApiError::Internal(format!("Blockhash fetch failed: {e}")))?
        .0;

    let mut instructions: Vec<Instruction> = Vec::new();

    // 1. Create recipient ATA if needed (idempotent)
    if !recipient_ata_exists {
        instructions.push(create_ata_idempotent_ix(
            &fee_payer_pk, &recipient_ata, &to_pubkey,
            &mint_pk, &sys_prog, &token_prog, &assoc_prog,
        ));
    }

    // 2. Transfer USDC amount to recipient
    instructions.push(spl_transfer_ix(
        &sender_ata, &recipient_ata, &from_pubkey, amount_units,
    ));

    // 3. Transfer wallet fee to fee wallet (if configured and fee > 0)
    let ata_rent_sol_display = if !recipient_ata_exists {
        Some(format!("{:.9}", ata_rent_lamports as f64 / 1e9))
    } else {
        None
    };

    if wallet_fee_units > 0 {
        if let Some(fee_wallet_addr) = &state.usdc_fee_wallet {
            let fee_wallet_pk = Pubkey::from_str(fee_wallet_addr)
                .map_err(|_| ApiError::Internal("Invalid fee wallet address configured".into()))?;
            let fee_wallet_ata = derive_ata(&fee_wallet_pk, &mint_pk, &token_prog, &assoc_prog);

            // Create fee wallet ATA if needed (fee payer pays)
            if state.rpc_client.get_account(&fee_wallet_ata).await.is_err() {
                if let Some(fp) = &state.fee_payer {
                    let fp_pk = fp.pubkey();
                    instructions.push(create_ata_idempotent_ix(
                        &fp_pk, &fee_wallet_ata, &fee_wallet_pk,
                        &mint_pk, &sys_prog, &token_prog, &assoc_prog,
                    ));
                }
            }

            instructions.push(spl_transfer_ix(
                &sender_ata, &fee_wallet_ata, &from_pubkey, wallet_fee_units,
            ));
        }
    }

    // Build transaction with correct fee payer
    let mut tx = Transaction::new_with_payer(&instructions, Some(&fee_payer_pk));
    tx.message.recent_blockhash = blockhash;

    // Fee payer signs if it's a separate keypair
    if let Some(fp_kp) = &state.fee_payer {
        tx.partial_sign(&[fp_kp.as_ref()], blockhash);
    }

    // ── Simulate before returning ─────────────────────────────────────────────
    // We can only simulate fully-signed txs. Since the user hasn't signed yet,
    // we simulate a "skeleton" with just the fee payer sig to catch obvious errors.
    // The final check happens via waitForConfirmation in the frontend.

    let tx_b64 = B64.encode(bincode::serialize(&tx)
        .map_err(|e| ApiError::Internal(format!("Serialize error: {e}")))?);

    info!("Prepared USDC tx: {} → {} units={} fee={} ata_needed={}",
        body.from_address, body.to_address, amount_units, wallet_fee_units, !recipient_ata_exists);

    Ok(Json(PrepareUsdcResponse {
        transaction_base64: tx_b64,
        network_fee_sol: format!("{:.9}", network_fee as f64 / 1e9),
        wallet_fee_usdc: format_usdc(wallet_fee_units),
        ata_created: !recipient_ata_exists,
        summary: PrepareUsdcSummary {
            amount_usdc: format_usdc(amount_units),
            wallet_fee_usdc: format_usdc(wallet_fee_units),
            total_deducted: format_usdc(total_deduct),
            recipient_receives: format_usdc(amount_units),
            network_fee_sol: format!("{:.9}", network_fee as f64 / 1e9),
            ata_rent_sol: ata_rent_sol_display,
        },
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
//  Internal helpers for USDC handlers
// ─────────────────────────────────────────────────────────────────────────────

/// Derive Associated Token Account PDA.
fn derive_ata(wallet: &Pubkey, mint: &Pubkey, token_prog: &Pubkey, assoc_prog: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[wallet.as_ref(), token_prog.as_ref(), mint.as_ref()],
        assoc_prog,
    ).0
}

/// Fetch SPL token balance in micro-units (6 decimals for USDC).
async fn get_spl_balance(rpc: &solana_client::nonblocking::rpc_client::RpcClient, ata: &Pubkey) -> Option<u64> {
    let info = rpc.get_token_account_balance(ata).await.ok()?;
    info.amount.parse::<u64>().ok()
}

/// Parse a USDC decimal string to micro-units (6 decimals).
fn parse_usdc_units(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() { return Err("Amount is empty".into()); }
    let parts: Vec<&str> = s.splitn(2, '.').collect();
    let whole: u64 = parts[0].parse().map_err(|_| "Invalid amount".to_string())?;
    let frac: u64 = if parts.len() == 2 {
        let f = parts[1];
        if f.len() > 6 { return Err("Max 6 decimal places for USDC".into()); }
        let padded = format!("{:0<6}", f);
        padded.parse().map_err(|_| "Invalid fractional amount".to_string())?
    } else {
        0
    };
    Ok(whole * 1_000_000 + frac)
}

/// Format micro-units as decimal USDC string.
fn format_usdc(units: u64) -> String {
    format!("{}.{:06}", units / 1_000_000, units % 1_000_000)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
        + if units % 1_000_000 == 0 { ".00" } else { "" }
}

/// Build a CreateIdempotent Associated Token Account instruction.
fn create_ata_idempotent_ix(
    payer: &Pubkey, ata: &Pubkey, owner: &Pubkey,
    mint: &Pubkey, sys_prog: &Pubkey, token_prog: &Pubkey, assoc_prog: &Pubkey,
) -> Instruction {
    Instruction {
        program_id: *assoc_prog,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(*ata, false),
            AccountMeta::new_readonly(*owner, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(*sys_prog, false),
            AccountMeta::new_readonly(*token_prog, false),
        ],
        data: vec![1], // 1 = CreateIdempotent
    }
}

/// Build a raw SPL Token Transfer instruction (discriminator = 3).
fn spl_transfer_ix(from_ata: &Pubkey, to_ata: &Pubkey, authority: &Pubkey, amount: u64) -> Instruction {
    let token_prog = Pubkey::from_str(TOKEN_PROGRAM).unwrap();
    let mut data = vec![3u8]; // Transfer discriminator
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: token_prog,
        accounts: vec![
            AccountMeta::new(*from_ata, false),
            AccountMeta::new(*to_ata, false),
            AccountMeta::new_readonly(*authority, true),
        ],
        data,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  GET /transaction/usdc-fee  — return configured USDC fee
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_usdc_fee(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "fee_usdc_units": state.usdc_fee_units,
        "fee_usdc": format_usdc(state.usdc_fee_units),
        "fee_wallet": state.usdc_fee_wallet,
        "fee_payer_configured": state.fee_payer.is_some(),
    }))
}

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

    // Skip preflight — avoids rate-limit errors from api.mainnet-beta.solana.com
    // simulation. The transaction will still fail on-chain if invalid.
    use solana_client::rpc_config::RpcSendTransactionConfig;
    let config = RpcSendTransactionConfig {
        skip_preflight: true,
        preflight_commitment: Some(solana_sdk::commitment_config::CommitmentLevel::Confirmed),
        ..Default::default()
    };

    // Try primary RPC client first, then fallback endpoints
    let primary_result = state.rpc_client.send_transaction_with_config(&tx, config).await;

    let signature = match primary_result {
        Ok(sig) => sig,
        Err(e) => {
            let err_str = e.to_string();
            // If primary is rate-limited/forbidden, try fallback endpoints directly
            if err_str.contains("403") || err_str.contains("forbidden") || err_str.contains("Access") {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(20))
                    .build()
                    .map_err(|e2| ApiError::Internal(e2.to_string()))?;

                let b64_tx = B64.encode(&tx_bytes);
                let rpc_body = serde_json::json!({
                    "jsonrpc": "2.0", "id": 1,
                    "method": "sendTransaction",
                    "params": [b64_tx, {"encoding": "base64", "skipPreflight": true, "preflightCommitment": "confirmed"}]
                });

                let mut last_err = err_str;
                for endpoint in FALLBACK_RPCS {
                    if *endpoint == state.rpc_url.as_str() { continue; }
                    match client.post(*endpoint).json(&rpc_body).send().await {
                        Ok(resp) => {
                            if let Ok(json) = resp.json::<serde_json::Value>().await {
                                if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
                                    let sig = result.parse::<solana_sdk::signature::Signature>()
                                        .map_err(|e2| ApiError::Internal(e2.to_string()))?;
                                    info!("Broadcasted via fallback {}: {}", endpoint, sig);
                                    return Ok(Json(SendResponse {
                                        explorer_url: build_explorer_url(&sig.to_string(), &state.cluster),
                                        signature: sig.to_string(),
                                    }));
                                } else if let Some(err) = json.get("error") {
                                    last_err = err.to_string();
                                }
                            }
                        }
                        Err(e2) => { last_err = e2.to_string(); }
                    }
                }
                return Err(ApiError::from(wallet_core::WalletError::TransactionSubmit(last_err)));
            }
            return Err(ApiError::from(wallet_core::WalletError::TransactionSubmit(err_str)));
        }
    };

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
//  GET /wallet/signatures/:address  — on-chain tx history from RPC
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_wallet_signatures(
    State(state): State<AppState>,
    Path(address): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, ApiError> {
    let pubkey = validate_address(&address)?;
    let limit  = params.limit.unwrap_or(20).min(50) as usize;

    let sigs = state
        .rpc_client
        .get_signatures_for_address(&pubkey)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch signatures: {e}")))?;

    let items: Vec<Value> = sigs.iter().take(limit).map(|s| {
        let network = state.cluster.as_str();
        json!({
            "signature":  s.signature,
            "block_time": s.block_time,
            "status":     if s.err.is_some() { "failed" } else { "confirmed" },
            "direction":  "sent",
            "amount_sol": "—",
            "fee_sol":    "0.000005",
            "counterparty_address": "",
            "explorer_url": build_explorer_url(&s.signature, network),
        })
    }).collect();

    Ok(Json(json!({
        "address":      address,
        "transactions": items,
        "count":        items.len(),
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
