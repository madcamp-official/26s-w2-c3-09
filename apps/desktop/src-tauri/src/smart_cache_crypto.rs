use std::fs;
use std::path::Path;

use aes_gcm::aead::rand_core::{OsRng, RngCore};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const KEYRING_SERVICE: &str = "com.mousekeeper.desktop";
const KEYRING_ACCOUNT: &str = "smart-cache-v1";
const KEY_VERSION: u8 = 1;
const KEY_ID_BYTES: usize = 16;
const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;
const TAG_BYTES: u64 = 16;
const ENVELOPE_MAGIC: &[u8; 4] = b"MKS1";

pub const SMART_CACHE_ENCRYPTION_ALGORITHM: &str = "AES-256-GCM";
pub const SMART_CACHE_ENCRYPTION_FORMAT: &str = "MKS1_NONCE_CIPHERTEXT_TAG";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SmartCacheEncryptionMetadata {
    pub algorithm: String,
    pub format: String,
    pub key_id: String,
    pub nonce_hex: String,
    pub plaintext_size_bytes: u64,
    pub plaintext_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncryptedSmartCacheObject {
    pub bytes: Vec<u8>,
    pub size_bytes: u64,
    pub sha256: String,
    pub metadata: SmartCacheEncryptionMetadata,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SmartCacheKeyMaterial {
    version: u8,
    key_id: String,
    key_hex: String,
}

pub fn encrypted_smart_cache_object_size(plaintext_size_bytes: u64) -> Result<u64, String> {
    plaintext_size_bytes
        .checked_add(u64::try_from(ENVELOPE_MAGIC.len()).unwrap_or(4))
        .and_then(|value| value.checked_add(u64::try_from(NONCE_BYTES).unwrap_or(12)))
        .and_then(|value| value.checked_add(TAG_BYTES))
        .ok_or_else(|| "smart cache object size overflow".to_string())
}

pub fn encrypt_smart_cache_file(
    path: &Path,
    expected_plaintext_size_bytes: u64,
) -> Result<EncryptedSmartCacheObject, String> {
    let key_material = load_or_create_key_material()?;
    let key = decode_hex_array::<KEY_BYTES>(&key_material.key_hex)?;
    let mut nonce = [0_u8; NONCE_BYTES];
    OsRng.fill_bytes(&mut nonce);
    let plaintext =
        fs::read(path).map_err(|error| format!("cannot read smart cache source file: {error}"))?;
    if u64::try_from(plaintext.len()).unwrap_or(u64::MAX) != expected_plaintext_size_bytes {
        return Err("smart cache source size changed before encryption".to_string());
    }
    encrypt_smart_cache_bytes_with_key(&plaintext, &key_material.key_id, &key, &nonce)
}

pub fn encrypt_smart_cache_bytes_with_key(
    plaintext: &[u8],
    key_id: &str,
    key: &[u8; KEY_BYTES],
    nonce: &[u8; NONCE_BYTES],
) -> Result<EncryptedSmartCacheObject, String> {
    if key_id.trim().len() < 16 {
        return Err("smart cache encryption key id is too short".to_string());
    }
    let plaintext_size_bytes = u64::try_from(plaintext.len())
        .map_err(|_| "smart cache plaintext is too large".to_string())?;
    let plaintext_sha256 = sha256_hex(plaintext);
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| "cannot initialize smart cache encryption key".to_string())?;
    let mut ciphertext_and_tag = cipher
        .encrypt(Nonce::from_slice(nonce), plaintext)
        .map_err(|_| "smart cache encryption failed".to_string())?;
    let mut bytes =
        Vec::with_capacity(ENVELOPE_MAGIC.len() + nonce.len() + ciphertext_and_tag.len());
    bytes.extend_from_slice(ENVELOPE_MAGIC);
    bytes.extend_from_slice(nonce);
    bytes.append(&mut ciphertext_and_tag);
    let size_bytes = u64::try_from(bytes.len())
        .map_err(|_| "smart cache ciphertext is too large".to_string())?;
    let sha256 = sha256_hex(&bytes);
    Ok(EncryptedSmartCacheObject {
        bytes,
        size_bytes,
        sha256,
        metadata: SmartCacheEncryptionMetadata {
            algorithm: SMART_CACHE_ENCRYPTION_ALGORITHM.to_string(),
            format: SMART_CACHE_ENCRYPTION_FORMAT.to_string(),
            key_id: key_id.to_string(),
            nonce_hex: hex_lower(nonce),
            plaintext_size_bytes,
            plaintext_sha256,
        },
    })
}

fn load_or_create_key_material() -> Result<SmartCacheKeyMaterial, String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|_| "UNCONFIGURED: smart cache encryption key store unavailable".to_string())?;
    match entry.get_password() {
        Ok(serialized) => parse_key_material(&serialized),
        Err(keyring::Error::NoEntry) => {
            let material = generate_key_material();
            let serialized = serde_json::to_string(&material)
                .map_err(|error| format!("cannot encode smart cache encryption key: {error}"))?;
            entry.set_password(&serialized).map_err(|_| {
                "UNCONFIGURED: cannot persist smart cache encryption key".to_string()
            })?;
            Ok(material)
        }
        Err(_) => Err("UNCONFIGURED: cannot load smart cache encryption key".to_string()),
    }
}

fn parse_key_material(serialized: &str) -> Result<SmartCacheKeyMaterial, String> {
    let material: SmartCacheKeyMaterial = serde_json::from_str(serialized)
        .map_err(|_| "UNCONFIGURED: smart cache encryption key is corrupt".to_string())?;
    if material.version != KEY_VERSION
        || material.key_id.trim().len() < 16
        || decode_hex_array::<KEY_BYTES>(&material.key_hex).is_err()
    {
        return Err("UNCONFIGURED: smart cache encryption key is invalid".to_string());
    }
    Ok(material)
}

fn generate_key_material() -> SmartCacheKeyMaterial {
    let mut key = [0_u8; KEY_BYTES];
    let mut key_id = [0_u8; KEY_ID_BYTES];
    OsRng.fill_bytes(&mut key);
    OsRng.fill_bytes(&mut key_id);
    SmartCacheKeyMaterial {
        version: KEY_VERSION,
        key_id: format!("mks1-{}", hex_lower(&key_id)),
        key_hex: hex_lower(&key),
    }
}

fn decode_hex_array<const N: usize>(value: &str) -> Result<[u8; N], String> {
    if value.len() != N * 2 {
        return Err("hex length mismatch".to_string());
    }
    let mut bytes = [0_u8; N];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_value(chunk[0]).ok_or_else(|| "invalid hex".to_string())?;
        let low = hex_value(chunk[1]).ok_or_else(|| "invalid hex".to_string())?;
        bytes[index] = (high << 4) | low;
    }
    Ok(bytes)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};

    use super::{
        encrypt_smart_cache_bytes_with_key, encrypted_smart_cache_object_size, ENVELOPE_MAGIC,
        NONCE_BYTES, SMART_CACHE_ENCRYPTION_ALGORITHM, SMART_CACHE_ENCRYPTION_FORMAT,
    };

    #[test]
    fn smart_cache_encryption_wraps_plaintext_in_authenticated_envelope() {
        let plaintext = b"mousekeeper cached file";
        let key = [7_u8; 32];
        let nonce = [3_u8; 12];
        let encrypted =
            encrypt_smart_cache_bytes_with_key(plaintext, "mks1-test-key-1234", &key, &nonce)
                .expect("encrypt");

        assert_eq!(
            encrypted.size_bytes,
            encrypted_smart_cache_object_size(plaintext.len() as u64).expect("size")
        );
        assert_ne!(encrypted.bytes, plaintext);
        assert_eq!(&encrypted.bytes[..ENVELOPE_MAGIC.len()], ENVELOPE_MAGIC);
        assert_eq!(
            encrypted.metadata.algorithm,
            SMART_CACHE_ENCRYPTION_ALGORITHM
        );
        assert_eq!(encrypted.metadata.format, SMART_CACHE_ENCRYPTION_FORMAT);
        assert_eq!(encrypted.metadata.nonce_hex, "030303030303030303030303");
        assert_eq!(
            encrypted.metadata.plaintext_size_bytes,
            plaintext.len() as u64
        );

        let cipher = Aes256Gcm::new_from_slice(&key).expect("cipher");
        let body = &encrypted.bytes[ENVELOPE_MAGIC.len() + NONCE_BYTES..];
        let decrypted = cipher
            .decrypt(Nonce::from_slice(&nonce), body)
            .expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }
}
