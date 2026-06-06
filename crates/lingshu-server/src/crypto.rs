//! AES-256-GCM encryption for integration tokens at rest.
//!
//! `SecurityConfig.encryption_key` is an arbitrary-length operator-supplied string;
//! SHA-256 derives a fixed 32-byte AES-256 key from it. Each encrypted blob is
//! `nonce (12 bytes) || ciphertext (includes the GCM authentication tag)`, so the
//! nonce travels with the data it protects and decryption is self-contained — no
//! separate nonce column is needed in the `integrations` table.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use sha2::{Digest, Sha256};

const NONCE_LEN: usize = 12;

/// Number of SHA-256 iterations for key stretching.
/// 100 000 rounds ≈ 100ms on modern hardware, making brute-force ~10⁵× harder
/// while staying imperceptible for the occasional integration creation.
const KEY_DERIVE_ROUNDS: usize = 100_000;

/// Derive a 32-byte AES-256 key from an arbitrary-length operator-supplied
/// string. Uses iterated SHA-256 (PBKDF2-like) so that even weak human-chosen
/// keys are significantly more expensive to brute-force.
fn derive_key(key: &str) -> [u8; 32] {
    let key_bytes = key.as_bytes();
    let mut state = Sha256::digest(key_bytes).to_vec(); // 32 bytes

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

fn build_cipher(key: &str) -> anyhow::Result<Aes256Gcm> {
    Aes256Gcm::new_from_slice(&derive_key(key))
        .map_err(|e| anyhow::anyhow!("failed to initialize cipher: {e}"))
}

/// Encrypts `plaintext` with a key derived from `key`. Returns `nonce || ciphertext`.
pub fn encrypt(plaintext: &str, key: &str) -> anyhow::Result<Vec<u8>> {
    let cipher = build_cipher(key)?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

    let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    Ok(blob)
}

/// Decrypts a `nonce || ciphertext` blob produced by [`encrypt`]. Returns `Err` if
/// the blob is too short, the key is wrong, or the ciphertext/tag was tampered with.
pub fn decrypt(blob: &[u8], key: &str) -> anyhow::Result<String> {
    if blob.len() < NONCE_LEN {
        anyhow::bail!(
            "ciphertext blob too short: expected at least {NONCE_LEN} bytes, got {}",
            blob.len()
        );
    }
    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = build_cipher(key)?
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("decryption failed: wrong key or tampered ciphertext"))?;

    String::from_utf8(plaintext)
        .map_err(|e| anyhow::anyhow!("decrypted bytes are not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "test-encryption-key";

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
}
