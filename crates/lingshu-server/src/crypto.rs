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

// ── Shared encryption key file (single source of truth across launch modes) ─
//
// The packaged Tauri app generates and persists a random key at
// `~/Library/Application Support/com.lingshu.desktop/sidecar-encryption-key`
// (see the matching `sidecar_encryption_key` logic in `src-tauri/src/main.rs`).
// When the backend runs natively (dev) with no explicit `ENCRYPTION_KEY` in
// env/config, it reads/creates the SAME file so every local launch mode
// (`cargo run`, `tauri dev`, packaged app) shares one key and can decrypt
// provider API keys saved by any other mode. Docker/Linux deployments are not
// covered (they supply `ENCRYPTION_KEY` via env), so this returns `None` there.

/// AES-256 key length in bytes; the generated key is this many random bytes hex-encoded.
#[cfg(any(target_os = "macos", test))]
const SHARED_KEY_LEN: usize = 32;

/// Load (or create) the shared encryption key. Returns `None` on non-macOS
/// platforms, where `ENCRYPTION_KEY` must be supplied via env/config instead.
#[cfg(target_os = "macos")]
pub fn load_or_create_shared_encryption_key() -> Option<String> {
    let path = shared_encryption_key_path()?;
    load_or_create_shared_encryption_key_at(&path, generate_shared_encryption_key)
}

/// Non-macOS: no shared file; rely on env/config `ENCRYPTION_KEY`.
#[cfg(not(target_os = "macos"))]
pub fn load_or_create_shared_encryption_key() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
const SHARED_KEY_IDENTIFIER: &str = "com.lingshu.desktop";
#[cfg(target_os = "macos")]
const SHARED_KEY_FILE: &str = "sidecar-encryption-key";

#[cfg(target_os = "macos")]
fn shared_encryption_key_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::Path::new(&home)
            .join("Library/Application Support")
            .join(SHARED_KEY_IDENTIFIER)
            .join(SHARED_KEY_FILE),
    )
}

/// Read an existing key file (trimmed, non-empty) or generate, persist (0600 on
/// unix), and return a new one. Path-injectable mirror of the Tauri-side helper
/// so it can be unit-tested without touching the real config dir.
#[cfg(any(target_os = "macos", test))]
fn load_or_create_shared_encryption_key_at(
    path: &std::path::Path,
    generate: impl FnOnce() -> Option<String>,
) -> Option<String> {
    if let Ok(existing) = std::fs::read_to_string(path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let key = generate()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    write_secret_file(path, &key).ok()?;
    Some(key)
}

#[cfg(any(target_os = "macos", test))]
fn write_secret_file(path: &std::path::Path, value: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(value.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
fn generate_shared_encryption_key() -> Option<String> {
    let mut bytes = [0u8; SHARED_KEY_LEN];
    rand::thread_rng().fill_bytes(&mut bytes);
    Some(encode_hex(&bytes))
}

#[cfg(any(target_os = "macos", test))]
fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
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

    // ── Shared encryption key file ──────────────────────────────

    fn unique_temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "lingshu-shared-key-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn shared_key_reuses_existing_file() {
        let path = unique_temp_path("existing");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "  existing-shared-key  \n").unwrap();

        let key = load_or_create_shared_encryption_key_at(&path, || {
            panic!("existing key must not be regenerated")
        })
        .expect("existing key");

        assert_eq!(key, "existing-shared-key");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn shared_key_creates_missing_file_with_generated_value() {
        let path = unique_temp_path("missing");
        let _ = std::fs::remove_file(&path);

        let key = load_or_create_shared_encryption_key_at(&path, generate_shared_encryption_key)
            .expect("generated key");

        // 32 random bytes hex-encoded
        assert_eq!(key.len(), SHARED_KEY_LEN * 2);
        assert!(key.bytes().all(|b| b.is_ascii_hexdigit()));
        // Persisted (newline-terminated) and reused on the next read.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), format!("{key}\n"));
        let reread =
            load_or_create_shared_encryption_key_at(&path, || panic!("must reuse persisted key"))
                .expect("reread key");
        assert_eq!(reread, key);
        let _ = std::fs::remove_file(path);
    }
}
