use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};

/// Derive a 32-byte AES key from a master key string.
fn derive_key(master_key: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(master_key.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypt plaintext using AES-256-GCM. Returns nonce + ciphertext.
pub fn encrypt(plaintext: &[u8], master_key: &str) -> Result<Vec<u8>, String> {
    let key = derive_key(master_key);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Key init error: {e}"))?;

    // Generate random 12-byte nonce
    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption error: {e}"))?;

    // Prepend nonce to ciphertext
    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypt data encrypted with `encrypt`. Expects nonce + ciphertext.
pub fn decrypt(data: &[u8], master_key: &str) -> Result<Vec<u8>, String> {
    if data.len() < 13 {
        return Err("Data too short".into());
    }

    let key = derive_key(master_key);
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Key init error: {e}"))?;

    let nonce = Nonce::from_slice(&data[..12]);
    let ciphertext = &data[12..];

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = "test-encryption-key-32-bytes-ok!";
        let plaintext = b"hello world";
        let encrypted = encrypt(plaintext, key).unwrap();
        let decrypted = decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
