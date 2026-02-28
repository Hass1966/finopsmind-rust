use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,       // user_id
    pub org_id: Uuid,
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    expiry_hours: u64,
}

impl JwtManager {
    pub fn new(secret: &str, expiry_hours: u64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            expiry_hours,
        }
    }

    pub fn generate_token(&self, user_id: Uuid, org_id: Uuid, email: &str, role: &str) -> Result<String, jsonwebtoken::errors::Error> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id,
            org_id,
            email: email.into(),
            role: role.into(),
            iat: now.timestamp(),
            exp: (now + Duration::hours(self.expiry_hours as i64)).timestamp(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let token_data = decode::<Claims>(token, &self.decoding_key, &Validation::default())?;
        Ok(token_data.claims)
    }
}

/// Generate a random 32-byte API key and return (hex_key, sha256_hash)
pub fn generate_api_key() -> (String, String) {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    let hex_key = hex::encode(key);
    let hash = hash_api_key(&hex_key);
    (hex_key, hash)
}

pub fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}
