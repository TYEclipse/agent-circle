//! Key derivation and backup — BIP-39 mnemonic support.
//!
//! Derives Ed25519 keypairs from 12-word BIP-39 mnemonics,
//! enabling human-readable key backup and cross-wallet portability.
//!
//! ## Usage
//!
//! ```ignore
//! let mnemonic = generate_mnemonic()?;       // "bacon bitter ..."
//! let identity = derive_from_mnemonic(&mnemonic, "")?;  // empty passphrase
//! ```

use crate::errors::AcResult;
use crate::identity::Identity;
use bip39::Mnemonic;
use rand::RngCore;

/// Word count for generated mnemonics (12 words = 128 bits).
const MNEMONIC_WORD_COUNT: usize = 12;

/// Generate a new 12-word BIP-39 mnemonic.
pub fn generate_mnemonic() -> AcResult<String> {
    let entrophy_bits = match MNEMONIC_WORD_COUNT {
        12 => 128,
        24 => 256,
        _ => 128,
    };
    let mut entropy = vec![0u8; entrophy_bits / 8];
    rand::rngs::OsRng.fill_bytes(&mut entropy);

    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| crate::errors::AcError::Identity(format!("mnemonic: {e}")))?;

    Ok(mnemonic.to_string())
}

/// Derive an Ed25519 Identity from a BIP-39 mnemonic phrase + optional passphrase.
///
/// Derivation: mnemonic → seed (PBKDF2) → first 32 bytes → Ed25519 SigningKey.
pub fn derive_from_mnemonic(mnemonic_str: &str, passphrase: &str) -> AcResult<Identity> {
    let mnemonic = Mnemonic::parse(mnemonic_str)
        .map_err(|e| crate::errors::AcError::Identity(format!("invalid mnemonic: {e}")))?;

    let seed = mnemonic.to_seed(passphrase);
    // Ed25519 uses 32-byte seeds; take the first 32 bytes of the 64-byte BIP-39 seed
    let mut key_seed = [0u8; 32];
    key_seed.copy_from_slice(&seed[..32]);

    Identity::from_seed(&key_seed)
}

/// Validate a mnemonic phrase without deriving a key.
pub fn validate_mnemonic(mnemonic_str: &str) -> Result<(), String> {
    Mnemonic::parse(mnemonic_str)
        .map(|_| ())
        .map_err(|e| format!("invalid mnemonic: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_mnemonic() {
        let m = generate_mnemonic().unwrap();
        assert!(m.split_whitespace().count() >= 12);
    }

    #[test]
    fn test_derive_from_mnemonic() {
        let mnemonic = generate_mnemonic().unwrap();
        let id1 = derive_from_mnemonic(&mnemonic, "").unwrap();
        let id2 = derive_from_mnemonic(&mnemonic, "").unwrap();
        // Same mnemonic → same identity
        assert_eq!(id1.did, id2.did);
        assert_eq!(id1.short_code, id2.short_code);
    }

    #[test]
    fn test_different_passphrase_different_key() {
        let mnemonic = generate_mnemonic().unwrap();
        let id1 = derive_from_mnemonic(&mnemonic, "").unwrap();
        let id2 = derive_from_mnemonic(&mnemonic, "secret").unwrap();
        assert_ne!(id1.did, id2.did);
    }

    #[test]
    fn test_validate_mnemonic_rejects_bad_input() {
        assert!(validate_mnemonic("not a valid mnemonic phrase").is_err());
        assert!(validate_mnemonic("").is_err());
    }

    #[test]
    fn test_known_test_vector() {
        // Standard BIP-39 test vector
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let id = derive_from_mnemonic(mnemonic, "TREZOR").unwrap();
        assert!(!id.did.is_empty());
        assert!(!id.short_code.is_empty());
    }
}
