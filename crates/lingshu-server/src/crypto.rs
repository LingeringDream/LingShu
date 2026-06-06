//! AES-256-GCM encryption for integration tokens at rest.
//!
//! `SecurityConfig.encryption_key` is an arbitrary-length operator-supplied string.
//! SHA-256 with 100 000 rounds derives a fixed 32-byte AES-256 key from it.
//! Each encrypted blob is `nonce (12 bytes) || ciphertext (includes the GCM
//! authentication tag)`, so the nonce travels with the data and decryption is
//! self-contained — no separate nonce column is needed.
//!
//! # Threat model
//!
//! LingShu is a **single-user local desktop app**. The encrypted tokens are stored
//! in a local PostgreSQL database. The `ENCRYPTION_KEY` lives in `.env` or
//! `config.toml` on the same machine. An attacker who can read the DB file can
//! almost certainly read the key material too, so this encryption is **not** a
//! defence against an attacker with filesystem access. Its purpose is narrower:
//!
//! 1. **Defence-in-depth** — a misconfigured backup or a log capture that includes
//!    the database but not the config won't expose plaintext tokens.
//! 2. **Prevent accidental exposure** — encrypted columns are explicitly excluded
//!    from API responses via `IntegrationResponse` and `SELECT` column lists;
//!    the ciphertext is never logged.
//!
//! For operators: use a high-entropy random key (≥ 32 bytes base64-encoded) and
//! keep it out of the database volume.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use sha2::{Digest, Sha256};

const NONCE_LEN: usize = 12;

/// Number of SHA-256 rounds for key derivation.
const KEY_DERIVE_ROUNDS: usize = 100_000;

/// A pre-initialized AES-256-GCM cipher that derives its key once at construction
/// time and reuses it across `encrypt` / `decrypt` calls. This avoids re-running
/// the 100k-round KDF on every operation.
///
/// Construct via [`TokenCipher::from_key_str`]. The inner [`Aes256Gcm`] is not
/// `Clone`, so callers should wrap this in an `Arc` when sharing across threads.
pub struct TokenCipher(Aes256Gcm);

impl TokenCipher {
    /// Derive an AES-256 key from `key` and initialize the cipher. The expensive
    /// KDF runs exactly once here.
    pub fn from_key_str(key: &str) -> anyhow::Result<Self> {
        let cipher = Aes256Gcm::new_from_slice(&derive_key(key))
            .map_err(|e| anyhow::anyhow!("failed to initialize TokenCipher: {e}"))?;
        Ok(Self(cipher))
    }

    /// Encrypt `plaintext`, returning `nonce (12B) || ciphertext (+ GCM tag)`.
    pub fn encrypt(&self, plaintext: &str) -> anyhow::Result<Vec<u8>> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .0
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

        let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    /// Decrypt a blob produced by [`TokenCipher::encrypt`]. Returns `Err` when
    /// the blob is too short, the key is wrong, or the data was tampered with.
    pub fn decrypt(&self, blob: &[u8]) -> anyhow::Result<String> {
        if blob.len() < NONCE_LEN {
            anyhow::bail!(
                "ciphertext blob too short: expected at least {NONCE_LEN} bytes, got {}",
                blob.len()
            );
        }
        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .0
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("decryption failed: wrong key or tampered ciphertext"))?;

        String::from_utf8(plaintext)
            .map_err(|e| anyhow::anyhow!("decrypted bytes are not valid UTF-8: {e}"))
    }
}

// ── Free-standing functions (used by tests and for one-shot operations) ─

/// Derive a 32-byte AES-256 key from an arbitrary-length string.
/// Uses iterated SHA-256 so that even short passphrases are moderately
/// expensive to brute-force. Still, prefer a high-entropy random key.
fn derive_key(key: &str) -> [u8; 32] {
    let key_bytes = key.as_bytes();
    let mut state = Sha256::digest(key_bytes).to_vec();

    for _ in 0..KEY_DERIVE_ROUNDS {
        let mut hasher = Sha256::new();
        hasher.update(&state);
        hasher.update(key_bytes);
        state = hasher.finalize().to_vec();
    }

    let mut out = [0u8; 32];
    out.copy_from_slice(&state);
    out
}

/// One-shot encrypt with an ad-hoc key string. Prefer [`TokenCipher`] when
/// encrypting more than once.
pub fn encrypt(plaintext: &str, key: &str) -> anyhow::Result<Vec<u8>> {
    TokenCipher::from_key_str(key)?.encrypt(plaintext)
}

/// One-shot decrypt with an ad-hoc key string. Prefer [`TokenCipher`] when
/// decrypting more than once.
pub fn decrypt(blob: &[u8], key: &str) -> anyhow::Result<String> {
    TokenCipher::from_key_str(key)?.decrypt(blob)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "test-encryption-key";

    // ── Free-function round-trip (one-shot) ────────────────────

    #[test]
    fn round_trip_recovers_plaintext() {
        let blob = encrypt("hello, soul ledger", KEY).expect("encrypt");
        let plaintext = decrypt(&blob, KEY).expect("decrypt");
        assert_eq!(plaintext, "hello, soul ledger");
    }

    #[test]
    fn empty_plaintext_round_trips() {
        let blob = encrypt("", KEY).expect("encrypt");
        assert_eq!(decrypt(&blob, KEY).expect("decrypt"), "");
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let blob = encrypt("super-secret-access-token", KEY).expect("encrypt");
        assert!(decrypt(&blob, "a-totally-different-key").is_err());
    }

    #[test]
    fn tampered_ciphertext_fails_to_decrypt() {
        let mut blob = encrypt("super-secret-access-token", KEY).expect("encrypt");
        let last = blob.len() - 1;
        blob[last] ^= 0xFF;
        assert!(decrypt(&blob, KEY).is_err());
    }

    #[test]
    fn tampered_nonce_fails_to_decrypt() {
        let mut blob = encrypt("super-secret-access-token", KEY).expect("encrypt");
        blob[0] ^= 0xFF;
        assert!(decrypt(&blob, KEY).is_err());
    }

    #[test]
    fn truncated_blob_fails_to_decrypt() {
        let blob = encrypt("super-secret-access-token", KEY).expect("encrypt");
        assert!(decrypt(&blob[..NONCE_LEN - 1], KEY).is_err());
        assert!(decrypt(&[], KEY).is_err());
    }

    #[test]
    fn nonces_differ_across_encryptions() {
        let a = encrypt("identical plaintext", KEY).expect("encrypt a");
        let b = encrypt("identical plaintext", KEY).expect("encrypt b");
        assert_ne!(
            &a[..NONCE_LEN],
            &b[..NONCE_LEN],
            "nonces must be freshly randomized per encryption"
        );
        assert_ne!(a, b, "ciphertexts should differ when nonces differ");
    }

    // ── TokenCipher (cached) tests ─────────────────────────────

    #[test]
    fn token_cipher_round_trip() {
        let cipher = TokenCipher::from_key_str(KEY).expect("TokenCipher::new");
        let blob = cipher.encrypt("cached-cipher-token").expect("encrypt");
        assert_eq!(
            cipher.decrypt(&blob).expect("decrypt"),
            "cached-cipher-token"
        );
    }

    #[test]
    fn token_cipher_wrong_key_fails() {
        let a = TokenCipher::from_key_str("key-a").expect("cipher a");
        let b = TokenCipher::from_key_str("key-b").expect("cipher b");
        let blob = a.encrypt("token").expect("encrypt a");
        assert!(b.decrypt(&blob).is_err());
    }

    #[test]
    fn token_cipher_tampered_blob_fails() {
        let cipher = TokenCipher::from_key_str(KEY).expect("cipher");
        let mut blob = cipher.encrypt("token").expect("encrypt");
        blob[0] ^= 0xFF;
        assert!(cipher.decrypt(&blob).is_err());
    }

    // ── KDF determinism ────────────────────────────────────────

    #[test]
    fn derive_key_is_deterministic() {
        let k1 = derive_key("my-secret-key");
        let k2 = derive_key("my-secret-key");
        assert_eq!(k1, k2, "same input must produce same key");
    }

    #[test]
    fn derive_key_different_inputs_produce_different_keys() {
        let k1 = derive_key("key-a");
        let k2 = derive_key("key-b");
        assert_ne!(k1, k2, "different inputs must produce different keys");
    }
}
