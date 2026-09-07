// wallet.rs — non-custodial wallet creation and import
//
// Security contract:
//  - Private keys NEVER leave this process.
//  - Private keys are NEVER stored in plaintext.
//  - All signing happens locally in this module.
//  - Seed bytes are zeroized immediately after key derivation.

use bip39::{Language, Mnemonic};
use ed25519_dalek_bip32::{DerivationPath, ExtendedSigningKey};
use rand::RngCore;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::str::FromStr;
use zeroize::Zeroize;

use crate::error::WalletError;

/// Solana BIP-44 derivation path: m/44'/501'/0'/0'
const SOLANA_DERIVATION_PATH: &str = "m/44'/501'/0'/0'";

/// An in-memory wallet. Never serialise or send over any network.
pub struct Wallet {
    keypair: Keypair,
}

impl Wallet {
    // ── Construction ────────────────────────────────────────────────────

    /// Generate a brand-new wallet from cryptographically secure randomness.
    /// Returns `(wallet, 24-word mnemonic)`.
    pub fn generate() -> Result<(Self, String), WalletError> {
        // 256 bits of OS entropy → 24-word BIP-39 mnemonic.
        let mut entropy = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut entropy);

        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
            .map_err(|e| WalletError::MnemonicGeneration(e.to_string()))?;

        let phrase = mnemonic.to_string();
        let wallet = Self::from_mnemonic(&phrase)?;
        Ok((wallet, phrase))
    }

    /// Import a wallet from a BIP-39 mnemonic phrase (12 or 24 words).
    pub fn from_mnemonic(phrase: &str) -> Result<Self, WalletError> {
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, phrase)
            .map_err(|e| WalletError::InvalidMnemonic(e.to_string()))?;

        // BIP-39 seed with empty passphrase — standard for Solana wallets.
        let mut seed: [u8; 64] = mnemonic.to_seed("");
        let keypair = keypair_from_seed_bytes(&seed)?;
        seed.zeroize();
        Ok(Wallet { keypair })
    }

    /// Import a wallet from a base58-encoded 64-byte keypair.
    /// This is the format exported by Phantom and Solana CLI.
    pub fn from_base58_private_key(base58: &str) -> Result<Self, WalletError> {
        let bytes = bs58::decode(base58)
            .into_vec()
            .map_err(|e| WalletError::InvalidPrivateKey(e.to_string()))?;
        let keypair = Keypair::try_from(bytes.as_slice())
            .map_err(|e| WalletError::InvalidPrivateKey(e.to_string()))?;
        Ok(Wallet { keypair })
    }

    // ── Accessors ────────────────────────────────────────────────────────

    /// Public key of this wallet.
    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    /// Solana address as a base58 string.
    pub fn address(&self) -> String {
        self.keypair.pubkey().to_string()
    }

    /// Borrow the keypair for signing — `pub(crate)` only.
    pub(crate) fn keypair(&self) -> &Keypair {
        &self.keypair
    }
}

/// Derive a Solana Keypair from a 64-byte BIP-39 seed via SLIP-0010 ed25519.
fn keypair_from_seed_bytes(seed: &[u8; 64]) -> Result<Keypair, WalletError> {
    let path = DerivationPath::from_str(SOLANA_DERIVATION_PATH)
        .map_err(|e| WalletError::KeyDerivation(e.to_string()))?;

    let extended = ExtendedSigningKey::from_seed(seed)
        .map_err(|e| WalletError::KeyDerivation(e.to_string()))?
        .derive(&path)
        .map_err(|e| WalletError::KeyDerivation(e.to_string()))?;

    // ed25519-dalek-bip32 v0.3: signing_key is a public field.
    // Solana Keypair format: [secret_key(32) || public_key(32)]
    let mut kp_bytes = [0u8; 64];
    kp_bytes[..32].copy_from_slice(&extended.signing_key.to_bytes());
    kp_bytes[32..].copy_from_slice(&extended.verifying_key().to_bytes());

    let result = Keypair::try_from(kp_bytes.as_ref())
        .map_err(|e| WalletError::KeyDerivation(e.to_string()));

    // Zero the buffer — it contains the raw secret key bytes.
    kp_bytes.zeroize();
    result
}

/// Validate a Solana base58 address.
pub fn validate_address(address: &str) -> Result<Pubkey, WalletError> {
    Pubkey::from_str(address)
        .map_err(|_| WalletError::InvalidAddress(address.to_string()))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_24_word_mnemonic() {
        let (wallet, mnemonic) = Wallet::generate().unwrap();
        assert_eq!(mnemonic.split_whitespace().count(), 24);
        assert!(wallet.address().len() >= 32);
    }

    #[test]
    fn mnemonic_roundtrip_same_address() {
        let (w1, phrase) = Wallet::generate().unwrap();
        let w2 = Wallet::from_mnemonic(&phrase).unwrap();
        assert_eq!(w1.address(), w2.address());
    }

    #[test]
    fn two_wallets_have_different_addresses() {
        let (w1, _) = Wallet::generate().unwrap();
        let (w2, _) = Wallet::generate().unwrap();
        assert_ne!(w1.address(), w2.address());
    }

    #[test]
    fn invalid_mnemonic_rejected() {
        assert!(Wallet::from_mnemonic("invalid mnemonic phrase here").is_err());
        assert!(Wallet::from_mnemonic("").is_err());
    }

    #[test]
    fn validate_known_good_address() {
        let addr = "9B5XszUGdMaxCZ7uSQhPzdks5ZQSmWxrmzCSvtJ6Ns6g";
        assert!(validate_address(addr).is_ok());
    }

    #[test]
    fn validate_bad_addresses() {
        assert!(validate_address("not_valid!!").is_err());
        assert!(validate_address("").is_err());
    }
}
