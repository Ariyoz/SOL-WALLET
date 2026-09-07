// integration.rs — wallet-core integration tests
//
// These tests run offline (no RPC calls) and verify the core wallet logic.
// For live Devnet tests, run with: cargo test --test integration -- --ignored

use wallet_core::{
    balance::{lamports_to_sol, parse_sol_to_lamports, sol_to_lamports},
    error::WalletError,
    fee::{validate_balance_for_transfer, FeeEstimate},
    transaction::{TransactionRecord, TransactionStatus, TransferPreview},
    wallet::{validate_address, Wallet},
};

// ─────────────────────────────────────────────────────────────────────────────
//  Wallet generation & import
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn wallet_generate_produces_24_word_mnemonic() {
    let (_, mnemonic) = Wallet::generate().unwrap();
    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    assert_eq!(words.len(), 24);
}

#[test]
fn wallet_generate_unique_each_time() {
    let (w1, _) = Wallet::generate().unwrap();
    let (w2, _) = Wallet::generate().unwrap();
    assert_ne!(w1.address(), w2.address());
}

#[test]
fn wallet_mnemonic_roundtrip_same_address() {
    let (wallet1, mnemonic) = Wallet::generate().unwrap();
    let wallet2 = Wallet::from_mnemonic(&mnemonic).unwrap();
    assert_eq!(wallet1.address(), wallet2.address());
}

#[test]
fn wallet_invalid_mnemonic_rejected() {
    assert!(Wallet::from_mnemonic("invalid mnemonic phrase here").is_err());
    assert!(Wallet::from_mnemonic("").is_err());
    // 23 words — one short
    assert!(Wallet::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon \
         abandon abandon abandon abandon abandon abandon abandon abandon \
         abandon abandon abandon abandon abandon abandon abandon"
    ).is_err());
}

#[test]
fn wallet_address_is_valid_base58_pubkey() {
    let (wallet, _) = Wallet::generate().unwrap();
    let addr = wallet.address();
    // Valid Solana addresses are 32–44 base58 chars
    assert!(addr.len() >= 32 && addr.len() <= 44);
    assert!(validate_address(&addr).is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
//  Address validation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_known_good_addresses() {
    let valid = [
        "9B5XszUGdMaxCZ7uSQhPzdks5ZQSmWxrmzCSvtJ6Ns6g",
        "So11111111111111111111111111111111111111112",
        "11111111111111111111111111111111",
    ];
    for addr in valid {
        assert!(validate_address(addr).is_ok(), "should accept: {addr}");
    }
}

#[test]
fn validate_bad_addresses_rejected() {
    let invalid = [
        "",
        "not_a_solana_address",
        "0x1234567890abcdef",          // Ethereum format
        "too short",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", // too long
    ];
    for addr in invalid {
        assert!(validate_address(addr).is_err(), "should reject: {addr}");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Lamport / SOL conversion
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn lamports_to_sol_conversion() {
    assert_eq!(lamports_to_sol(1_000_000_000), 1.0);
    assert_eq!(lamports_to_sol(500_000_000),   0.5);
    assert_eq!(lamports_to_sol(5_000),         0.000005);
    assert_eq!(lamports_to_sol(1),             0.000_000_001);
    assert_eq!(lamports_to_sol(0),             0.0);
}

#[test]
fn parse_sol_string_to_lamports() {
    assert_eq!(parse_sol_to_lamports("1").unwrap(),           1_000_000_000);
    assert_eq!(parse_sol_to_lamports("0.1").unwrap(),         100_000_000);
    assert_eq!(parse_sol_to_lamports("0.01").unwrap(),        10_000_000);
    assert_eq!(parse_sol_to_lamports("0.001").unwrap(),       1_000_000);
    assert_eq!(parse_sol_to_lamports("0.0001").unwrap(),      100_000);
    assert_eq!(parse_sol_to_lamports("0.0005").unwrap(),      500_000);
    assert_eq!(parse_sol_to_lamports("0.000005").unwrap(),    5_000);
    assert_eq!(parse_sol_to_lamports("0.000000001").unwrap(), 1);
    assert_eq!(parse_sol_to_lamports("1.5").unwrap(),         1_500_000_000);
    assert_eq!(parse_sol_to_lamports("10").unwrap(),          10_000_000_000);
}

#[test]
fn parse_sol_micro_payment_amounts() {
    // All the micro-payment amounts from the spec
    assert_eq!(parse_sol_to_lamports("0.0001").unwrap(), 100_000);
    assert_eq!(parse_sol_to_lamports("0.0005").unwrap(), 500_000);
    assert_eq!(parse_sol_to_lamports("0.001").unwrap(),  1_000_000);
    assert_eq!(parse_sol_to_lamports("0.005").unwrap(),  5_000_000);
    assert_eq!(parse_sol_to_lamports("0.01").unwrap(),   10_000_000);
}

#[test]
fn parse_sol_zero_rejected() {
    assert!(parse_sol_to_lamports("0").is_err());
    assert!(parse_sol_to_lamports("0.000000000").is_err());
    assert!(parse_sol_to_lamports("0.00").is_err());
}

#[test]
fn parse_sol_invalid_rejected() {
    assert!(parse_sol_to_lamports("").is_err());
    assert!(parse_sol_to_lamports("abc").is_err());
    assert!(parse_sol_to_lamports("-1").is_err());
    assert!(parse_sol_to_lamports("1.0000000001").is_err()); // 10 decimals
}

#[test]
fn sol_to_lamports_roundtrip() {
    assert_eq!(sol_to_lamports(1.0).unwrap(),   1_000_000_000);
    assert_eq!(sol_to_lamports(0.001).unwrap(), 1_000_000);
    assert!(sol_to_lamports(0.0).is_err());
    assert!(sol_to_lamports(-0.5).is_err());
}

// ─────────────────────────────────────────────────────────────────────────────
//  Fee estimation and balance validation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn fee_estimate_no_service_fee() {
    let est = FeeEstimate::new(5_000, None);
    assert_eq!(est.service_fee_lamports, 0, "v1 must have zero service fee");
    assert_eq!(est.total_fee_lamports, est.network_fee_lamports);
}

#[test]
fn fee_estimate_usd_display() {
    let est = FeeEstimate::new(5_000, Some(200.0));
    // 5000 lamports / 1e9 * $200 = $0.000001
    let usd = est.network_fee_usd.unwrap();
    assert!((usd - 0.000001).abs() < 1e-12);
    assert!(est.network_fee_usd.is_some());
}

#[test]
fn fee_estimate_no_price_gives_no_usd() {
    let est = FeeEstimate::new(5_000, None);
    assert!(est.network_fee_usd.is_none());
}

#[test]
fn balance_validation_sufficient() {
    // 0.002 SOL balance, send 0.001 + 0.000005 fee → OK
    assert!(validate_balance_for_transfer(2_000_000, 1_000_000, 5_000).is_ok());
}

#[test]
fn balance_validation_exact_passes() {
    // balance = amount + fee exactly
    assert!(validate_balance_for_transfer(1_005_000, 1_000_000, 5_000).is_ok());
}

#[test]
fn balance_validation_one_lamport_short_fails() {
    assert!(matches!(
        validate_balance_for_transfer(1_004_999, 1_000_000, 5_000),
        Err(WalletError::InsufficientFunds { have_lamports: 1_004_999, need_lamports: 1_005_000 })
    ));
}

#[test]
fn balance_validation_zero_balance_fails() {
    assert!(matches!(
        validate_balance_for_transfer(0, 1_000, 5_000),
        Err(WalletError::InsufficientFunds { .. })
    ));
}

#[test]
fn insufficient_funds_error_message_is_user_friendly() {
    let err = validate_balance_for_transfer(1_000_000, 1_000_000, 5_000).unwrap_err();
    let msg = err.user_message();
    assert!(msg.contains("Insufficient SOL"));
    assert!(msg.contains("SOL"));
    assert!(!msg.contains("lamports"), "user message should not expose lamports");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Transaction records
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn transaction_record_explorer_url_devnet() {
    let record = make_record("sig123");
    let url = record.explorer_url("devnet");
    assert!(url.contains("sig123"));
    assert!(url.contains("cluster=devnet"));
    assert!(url.contains("explorer.solana.com"));
}

#[test]
fn transaction_record_explorer_url_mainnet_no_cluster_param() {
    let record = make_record("sig456");
    let url = record.explorer_url("mainnet-beta");
    assert!(url.contains("sig456"));
    assert!(!url.contains("cluster="));
}

#[test]
fn transaction_record_sol_display() {
    let r = make_record("x");
    // 1_000_000 lamports = 0.001 SOL
    assert!((r.amount_sol() - 0.001).abs() < 1e-12);
    // 5_000 lamports = 0.000005 SOL
    assert!((r.fee_sol() - 0.000005).abs() < 1e-12);
}

#[test]
fn transfer_preview_display_strings() {
    let fee = FeeEstimate::new(5_000, None);
    let preview = TransferPreview {
        recipient: "recipient".into(),
        amount_lamports: 1_000_000,
        fee_estimate: fee,
        total_lamports: 1_005_000,
        sender_balance_lamports: 10_000_000,
    };
    assert_eq!(preview.amount_sol_display(), "0.001000000");
    assert_eq!(preview.total_sol_display(),  "0.001005000");
    assert_eq!(preview.balance_sol_display(), "0.010000000");
}

// ─────────────────────────────────────────────────────────────────────────────
//  Error messages must be user-friendly (no internal details)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn error_messages_do_not_expose_internals() {
    let errors = [
        WalletError::InvalidAddress("bad_addr".into()),
        WalletError::InvalidAmount("nan".into()),
        WalletError::RpcError("connection refused".into()),
        WalletError::ConfirmationTimeout,
        WalletError::BlockhashExpired,
    ];
    for err in &errors {
        let msg = err.user_message();
        assert!(!msg.is_empty(), "user message must not be empty");
        // Should not contain raw Rust error strings
        assert!(!msg.contains("unwrap"), "must not expose Rust internals");
        assert!(!msg.contains("panic"),  "must not expose panics");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn make_record(sig: &str) -> TransactionRecord {
    TransactionRecord {
        signature: sig.into(),
        from_address: "FromAddr111111111111111111111111".into(),
        to_address: "ToAddr11111111111111111111111111".into(),
        amount_lamports: 1_000_000,
        fee_lamports: 5_000,
        status: TransactionStatus::Confirmed,
        timestamp: 0,
        slot: Some(42),
    }
}
