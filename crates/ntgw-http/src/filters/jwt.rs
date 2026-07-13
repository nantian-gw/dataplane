use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use ntgw_ir::{ClaimToHeader, JwtAuthFilter};
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use parking_lot::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) enum JwtError {
    MissingToken,
    InvalidToken(String),
    NoMatchingKey(String),
    Expired,
    JwksFetchFailed(String),
}

impl fmt::Display for JwtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JwtError::MissingToken => write!(f, "missing JWT token"),
            JwtError::InvalidToken(msg) => write!(f, "invalid JWT token: {msg}"),
            JwtError::NoMatchingKey(kid) => write!(f, "no matching JWK for kid: {kid}"),
            JwtError::Expired => write!(f, "JWT token expired"),
            JwtError::JwksFetchFailed(msg) => write!(f, "JWKS fetch failed: {msg}"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Deserialize)]
struct JwkKey {
    kid: String,
    kty: String,
    #[allow(dead_code)]
    alg: Option<String>,
    n: String,
    e: String,
}

struct KeyCache {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Instant,
}

pub(crate) struct JwtValidator {
    jwks_url: String,
    issuer: Option<String>,
    audience: Option<String>,
    cache: RwLock<KeyCache>,
    cache_ttl: Duration,
    http_client: reqwest::Client,
}

impl JwtValidator {
    pub(crate) fn new(filter: &JwtAuthFilter) -> Result<Self, JwtError> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| JwtError::JwksFetchFailed(format!("failed to build HTTP client: {e}")))?;

        let cache_ttl = Duration::from_secs(filter.cache_ttl_secs.unwrap_or(3600));

        Ok(Self {
            jwks_url: filter.jwks_url.clone(),
            issuer: filter.issuer.clone(),
            audience: filter.audience.clone(),
            cache: RwLock::new(KeyCache {
                keys: HashMap::new(),
                // Force initial JWKS fetch on first use by backdating the cache timestamp
                fetched_at: Instant::now() - cache_ttl,
            }),
            cache_ttl,
            http_client,
        })
    }

    fn needs_refresh(&self) -> bool {
        self.cache.read().fetched_at.elapsed() >= self.cache_ttl
    }

    fn get_key(&self, kid: &str) -> Result<Option<DecodingKey>, JwtError> {
        Ok(self.cache.read().keys.get(kid).cloned())
    }

    async fn get_key_with_refresh(&self, kid: &str) -> Result<DecodingKey, JwtError> {
        if let Some(key) = self.get_key(kid)? {
            return Ok(key);
        }
        self.fetch_and_cache_jwks().await?;
        self.get_key(kid)?
            .ok_or_else(|| JwtError::NoMatchingKey(kid.to_string()))
    }

    pub(crate) async fn validate(
        &self,
        token: &str,
        claims_to_headers: &[ClaimToHeader],
    ) -> Result<HashMap<String, String>, JwtError> {
        if token.is_empty() {
            return Err(JwtError::MissingToken);
        }

        if self.needs_refresh() {
            self.fetch_and_cache_jwks().await?;
        }

        let header = decode_header(token)
            .map_err(|e| JwtError::InvalidToken(format!("failed to decode header: {e}")))?;

        let kid = header.kid.ok_or_else(|| {
            JwtError::InvalidToken("token header missing 'kid' claim".to_string())
        })?;

        let key = self.get_key_with_refresh(&kid).await?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.algorithms = vec![Algorithm::RS256];
        validation.validate_exp = true;

        if let Some(ref issuer) = self.issuer {
            validation.set_issuer(&[issuer]);
        }
        if let Some(ref audience) = self.audience {
            validation.set_audience(&[audience]);
        }

        let token_data = match decode::<serde_json::Value>(token, &key, &validation) {
            Ok(data) => data,
            Err(e) => match e.kind() {
                ErrorKind::ExpiredSignature => return Err(JwtError::Expired),
                _ => return Err(JwtError::InvalidToken(e.to_string())),
            },
        };

        let mut headers = HashMap::new();
        if let Some(claims) = token_data.claims.as_object() {
            for mapping in claims_to_headers {
                if let Some(value) = claims.get(&mapping.claim) {
                    let header_value = match value {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    headers.insert(mapping.header.clone(), header_value);
                }
            }
        }

        Ok(headers)
    }

    async fn fetch_and_cache_jwks(&self) -> Result<(), JwtError> {
        let response = self
            .http_client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| JwtError::JwksFetchFailed(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(JwtError::JwksFetchFailed(format!(
                "HTTP {} from JWKS endpoint",
                response.status()
            )));
        }

        let jwks: JwksResponse = response
            .json()
            .await
            .map_err(|e| JwtError::JwksFetchFailed(format!("failed to parse JWKS: {e}")))?;

        let mut keys = HashMap::new();
        for key in &jwks.keys {
            if key.kty != "RSA" {
                continue;
            }
            match DecodingKey::from_rsa_components(&key.n, &key.e) {
                Ok(decoding_key) => {
                    keys.insert(key.kid.clone(), decoding_key);
                }
                Err(e) => {
                    // Skip invalid keys and continue — a subsequent key may be valid
                    tracing::warn!(
                        kid = %key.kid,
                        error = %e,
                        "failed to create decoding key from JWK",
                    );
                }
            }
        }

        if keys.is_empty() {
            return Err(JwtError::JwksFetchFailed(
                "no valid RSA keys in JWKS response".to_string(),
            ));
        }

        let mut cache = self.cache.write();
        cache.keys = keys;
        cache.fetched_at = Instant::now();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
