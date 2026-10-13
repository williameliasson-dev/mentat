//! Client-side keys and encryption, shared by `tui` and the `sync` tests.
//!
//! Everything derives from one [`AccountKey`], generated on the client and
//! never sent anywhere:
//!
//! - [`AccountKey::auth_token`] is the only thing the server sees.
//! - [`AccountKey::vault_key`] encrypts a vault and never leaves the device.
//!
//! Both are one-way HKDF derivations, so holding a token, or being the
//! server, gets you ciphertext and nothing more.

use std::fmt;
use std::str::FromStr;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;
use thiserror::Error;
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

// Changing either of these changes every key derived from it, making all
// existing accounts and vaults unreadable. Add a v2 instead.
const AUTH_INFO: &[u8] = b"mentat/v1/auth";
const VAULT_INFO: &[u8] = b"mentat/v1/vault/";

const NONCE_LEN: usize = 24;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("an account key is 64 hex characters")]
    InvalidAccountKey,

    /// Wrong key, wrong context, or tampered bytes. One error on purpose, so
    /// a failure says nothing about which.
    #[error("could not decrypt")]
    Decrypt,
}

/// The root secret of an account: it reads everything, and nothing else can.
///
/// `Display` / `FromStr` give the 64-hex-character form the user saves.
/// `Debug` is redacted so it cannot reach a log by accident.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct AccountKey([u8; 32]);

impl AccountKey {
    pub fn generate() -> Self {
        Self(random())
    }

    /// The sync server's bearer token, as 64 hex characters.
    pub fn auth_token(&self) -> String {
        hex::encode(self.derive(AUTH_INFO))
    }

    /// One key per vault, so ciphertext moved between vaults fails to open.
    pub fn vault_key(&self, vault: Uuid) -> VaultKey {
        VaultKey(self.derive(&[VAULT_INFO, vault.as_bytes()].concat()))
    }

    fn derive(&self, info: &[u8]) -> [u8; 32] {
        let mut key = [0; 32];
        Hkdf::<Sha256>::new(None, &self.0)
            .expand(info, &mut key)
            .expect("32 bytes is a valid HKDF-SHA256 length");
        key
    }
}

impl fmt::Display for AccountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

impl FromStr for AccountKey {
    type Err = CryptoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut key = [0; 32];
        hex::decode_to_slice(s.trim(), &mut key).map_err(|_| CryptoError::InvalidAccountKey)?;
        Ok(Self(key))
    }
}

impl fmt::Debug for AccountKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccountKey(<redacted>)")
    }
}

/// Encrypts one vault's contents with XChaCha20-Poly1305.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct VaultKey([u8; 32]);

impl VaultKey {
    /// Encrypts `plaintext` as `nonce || ciphertext || tag`.
    ///
    /// `context` says what the bytes are (say, "batch from device X"). It is
    /// authenticated, not encrypted, and [`open`](Self::open) needs the same
    /// one, so the server cannot pass one object's ciphertext off as another.
    pub fn seal(&self, plaintext: &[u8], context: &[u8]) -> Vec<u8> {
        let nonce: [u8; NONCE_LEN] = random();
        let payload = Payload {
            msg: plaintext,
            aad: context,
        };
        let ciphertext = self
            .cipher()
            .encrypt(&XNonce::from(nonce), payload)
            .expect("plaintext within XChaCha20-Poly1305 limits");
        [&nonce[..], &ciphertext].concat()
    }

    /// Reverses [`seal`](Self::seal), given the same `context`.
    pub fn open(&self, sealed: &[u8], context: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let (nonce, ciphertext) = sealed
            .split_first_chunk::<NONCE_LEN>()
            .ok_or(CryptoError::Decrypt)?;
        let payload = Payload {
            msg: ciphertext,
            aad: context,
        };
        self.cipher()
            .decrypt(&XNonce::from(*nonce), payload)
            .map_err(|_| CryptoError::Decrypt)
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new_from_slice(&self.0).expect("32-byte key")
    }
}

impl fmt::Debug for VaultKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VaultKey(<redacted>)")
    }
}

fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("os random number generator");
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(byte: u8) -> AccountKey {
        AccountKey([byte; 32])
    }

    fn vault(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn derivation_is_pinned() {
        // Every token and vault key in existence depends on these exact
        // outputs; if this fails, existing data just became unreadable. The
        // values are RFC 5869 HKDF-SHA256 over 32 bytes of 7, checked against
        // an independent implementation (Python's hmac), not copied from ours.
        let key = account(7);
        assert_eq!(
            key.auth_token(),
            "817f0dfcdb45da12aa6ca8e529712a60da1ccee624bf6b9585435575681c5f6e"
        );
        assert_eq!(
            hex::encode(key.vault_key(Uuid::nil()).0),
            "294bcadfaf899798d42a7349088af8377f8b5b904abe09c6e28bc56dc71aea55"
        );
    }

    #[test]
    fn derived_keys_are_independent() {
        let key = account(7);
        assert_ne!(key.vault_key(vault(1)).0, key.vault_key(vault(2)).0);
        assert_ne!(hex::encode(key.vault_key(vault(1)).0), key.auth_token());
        assert_ne!(account(7).auth_token(), account(8).auth_token());
    }

    #[test]
    fn seal_round_trips() {
        let key = account(1).vault_key(vault(1));
        let sealed = key.seal(b"note body", b"ctx");
        assert_eq!(key.open(&sealed, b"ctx").unwrap(), b"note body");
        // Random nonces: the same plaintext never seals the same way twice.
        assert_ne!(sealed, key.seal(b"note body", b"ctx"));
    }

    #[test]
    fn open_rejects_anything_else() {
        let key = account(1).vault_key(vault(1));
        let sealed = key.seal(b"note body", b"ctx");
        let mut tampered = sealed.clone();
        *tampered.last_mut().unwrap() ^= 1;

        let attempts = [
            (&key, &sealed[..], &b"other ctx"[..]),
            (&account(1).vault_key(vault(2)), &sealed, b"ctx"),
            (&account(2).vault_key(vault(1)), &sealed, b"ctx"),
            (&key, &tampered, b"ctx"),
            (&key, &sealed[..10], b"ctx"),
        ];
        for (key, sealed, context) in attempts {
            assert_eq!(key.open(sealed, context), Err(CryptoError::Decrypt));
        }
    }

    #[test]
    fn account_key_text_round_trips() {
        let key = AccountKey::generate();
        let text = key.to_string();
        assert_eq!(text.parse::<AccountKey>().unwrap().to_string(), text);
        assert_eq!(
            "nope".parse::<AccountKey>().unwrap_err(),
            CryptoError::InvalidAccountKey
        );
        assert_eq!(format!("{key:?}"), "AccountKey(<redacted>)");
    }
}
