use super::*;
use axum::{Router, routing::get, serve};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use ntgw_ir::ClaimToHeader;
use rand::rngs::OsRng;
use rsa::RsaPrivateKey;
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use std::net::SocketAddr;
use tokio::net::TcpListener;

struct TestKeypair {
    private_key_pem: String,
    jwk_n: String,
    jwk_e: String,
    kid: String,
}

impl TestKeypair {
    fn generate() -> Self {
        let mut rng = OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("failed to generate RSA key");
        let public_key = private_key.to_public_key();

        let private_key_pem = private_key
            .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
            .expect("failed to encode private key to PEM");

        let jwk_n = URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
        let jwk_e = URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());

        Self {
            private_key_pem: private_key_pem.to_string(),
            jwk_n,
            jwk_e,
            kid: "test-key-1".to_string(),
        }
    }

    fn sign_token(&self, claims: &serde_json::Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.kid.clone());
        let key = EncodingKey::from_rsa_pem(self.private_key_pem.as_bytes())
            .expect("failed to parse private key");
        jsonwebtoken::encode(&header, claims, &key).expect("failed to sign token")
    }

    fn sign_token_with_kid(&self, claims: &serde_json::Value, kid: &str) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.to_string());
        let key = EncodingKey::from_rsa_pem(self.private_key_pem.as_bytes())
            .expect("failed to parse private key");
        jsonwebtoken::encode(&header, claims, &key).expect("failed to sign token")
    }

    fn jwks_json(&self) -> String {
        serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "kid": self.kid,
                "n": self.jwk_n,
                "e": self.jwk_e,
                "alg": "RS256"
            }]
        })
        .to_string()
    }
}

async fn start_jwks_server(keypair: &TestKeypair) -> SocketAddr {
    let jwks_body = keypair.jwks_json();
    let app = Router::new().route(
        "/.well-known/jwks.json",
        get(move || {
            let body = jwks_body.clone();
            async move {
                axum::response::Json(serde_json::from_str::<serde_json::Value>(&body).unwrap())
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        serve(listener, app).await.unwrap();
    });
    addr
}

fn make_jwt_auth_filter(jwks_url: &str) -> JwtAuthFilter {
    JwtAuthFilter {
        jwks_url: jwks_url.to_string(),
        issuer: None,
        audience: None,
        header_name: "Authorization".to_string(),
        token_prefix: "Bearer ".to_string(),
        claims_to_headers: vec![],
        cache_ttl_secs: Some(3600),
    }
}

#[tokio::test]
async fn test_validate_valid_token() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/.well-known/jwks.json");

    let filter = make_jwt_auth_filter(&jwks_url);
    let validator = JwtValidator::new(&filter).unwrap();

    let claims = serde_json::json!({"sub": "user-123", "role": "admin", "exp": 9999999999_i64});
    let token = keypair.sign_token(&claims);

    let result = validator
        .validate(
            &token,
            &[ClaimToHeader {
                claim: "sub".to_string(),
                header: "X-Auth-User".to_string(),
            }],
        )
        .await;

    assert!(result.is_ok(), "expected valid token, got: {result:?}");
    let headers = result.unwrap();
    assert_eq!(
        headers.get("X-Auth-User").map(String::as_str),
        Some("user-123")
    );
}

#[tokio::test]
async fn test_validate_missing_token() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/.well-known/jwks.json");

    let filter = make_jwt_auth_filter(&jwks_url);
    let validator = JwtValidator::new(&filter).unwrap();

    let result = validator.validate("", &[]).await;
    assert!(matches!(result, Err(JwtError::MissingToken)));
}

#[tokio::test]
async fn test_validate_invalid_token() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/.well-known/jwks.json");

    let filter = make_jwt_auth_filter(&jwks_url);
    let validator = JwtValidator::new(&filter).unwrap();

    let result = validator.validate("not-a-valid-jwt-token", &[]).await;
    assert!(matches!(result, Err(JwtError::InvalidToken(_))));
}

#[tokio::test]
async fn test_validate_wrong_issuer() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/.well-known/jwks.json");

    let mut filter = make_jwt_auth_filter(&jwks_url);
    filter.issuer = Some("https://expected-issuer.example.com".to_string());

    let validator = JwtValidator::new(&filter).unwrap();

    let claims =
        serde_json::json!({"iss": "https://wrong-issuer.example.com", "exp": 9999999999_i64});
    let token = keypair.sign_token(&claims);

    let result = validator.validate(&token, &[]).await;
    assert!(
        matches!(result, Err(JwtError::InvalidToken(_))),
        "expected InvalidToken for wrong issuer, got: {result:?}"
    );
}

#[tokio::test]
async fn test_validate_wrong_audience() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/.well-known/jwks.json");

    let mut filter = make_jwt_auth_filter(&jwks_url);
    filter.audience = Some("expected-audience".to_string());

    let validator = JwtValidator::new(&filter).unwrap();

    let claims = serde_json::json!({"aud": "wrong-audience", "exp": 9999999999_i64});
    let token = keypair.sign_token(&claims);

    let result = validator.validate(&token, &[]).await;
    assert!(
        matches!(result, Err(JwtError::InvalidToken(_))),
        "expected InvalidToken for wrong audience, got: {result:?}"
    );
}

#[tokio::test]
async fn test_validate_expired_token() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/.well-known/jwks.json");

    let filter = make_jwt_auth_filter(&jwks_url);
    let validator = JwtValidator::new(&filter).unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = serde_json::json!({
        "exp": now - 3600,
        "sub": "user-123"
    });
    let token = keypair.sign_token(&claims);

    let result = validator.validate(&token, &[]).await;
    assert!(
        matches!(result, Err(JwtError::Expired)),
        "expected Expired, got: {result:?}"
    );
}

#[tokio::test]
async fn test_no_matching_key() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/.well-known/jwks.json");

    let filter = make_jwt_auth_filter(&jwks_url);
    let validator = JwtValidator::new(&filter).unwrap();

    let claims = serde_json::json!({"sub": "user-123", "exp": 9999999999_i64});
    let token = keypair.sign_token_with_kid(&claims, "unknown-kid");

    let result = validator.validate(&token, &[]).await;
    assert!(matches!(result, Err(JwtError::NoMatchingKey(_))));
}

#[tokio::test]
async fn test_claims_to_headers_mapping() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/.well-known/jwks.json");

    let filter = make_jwt_auth_filter(&jwks_url);
    let validator = JwtValidator::new(&filter).unwrap();

    let claims = serde_json::json!({
        "sub": "user-456",
        "email": "user@example.com",
        "role": "admin",
        "scope": 123
    , "exp": 9999999999_i64});
    let token = keypair.sign_token(&claims);

    let claim_mappings = vec![
        ClaimToHeader {
            claim: "sub".to_string(),
            header: "X-Auth-User".to_string(),
        },
        ClaimToHeader {
            claim: "email".to_string(),
            header: "X-Auth-Email".to_string(),
        },
        ClaimToHeader {
            claim: "role".to_string(),
            header: "X-Auth-Role".to_string(),
        },
        ClaimToHeader {
            claim: "scope".to_string(),
            header: "X-Auth-Scope".to_string(),
        },
        ClaimToHeader {
            claim: "nonexistent".to_string(),
            header: "X-Not-Present".to_string(),
        },
    ];

    let result = validator.validate(&token, &claim_mappings).await;
    assert!(result.is_ok());

    let headers = result.unwrap();
    assert_eq!(
        headers.get("X-Auth-User").map(String::as_str),
        Some("user-456")
    );
    assert_eq!(
        headers.get("X-Auth-Email").map(String::as_str),
        Some("user@example.com")
    );
    assert_eq!(
        headers.get("X-Auth-Role").map(String::as_str),
        Some("admin")
    );
    assert_eq!(headers.get("X-Auth-Scope").map(String::as_str), Some("123"));
    assert!(!headers.contains_key("X-Not-Present"));
}

#[tokio::test]
async fn test_jwks_fetch_failure() {
    let filter = JwtAuthFilter {
        jwks_url: "http://127.0.0.1:1/.well-known/jwks.json".to_string(),
        issuer: None,
        audience: None,
        header_name: "Authorization".to_string(),
        token_prefix: "Bearer ".to_string(),
        claims_to_headers: vec![],
        cache_ttl_secs: Some(3600),
    };

    let validator = JwtValidator::new(&filter).unwrap();

    let keypair = TestKeypair::generate();
    let claims = serde_json::json!({"sub": "user-123", "exp": 9999999999_i64});
    let token = keypair.sign_token(&claims);

    let result = validator.validate(&token, &[]).await;
    assert!(
        matches!(result, Err(JwtError::JwksFetchFailed(_))),
        "expected JwksFetchFailed, got: {result:?}"
    );
}

#[tokio::test]
async fn test_jwks_http_error_status() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/nonexistent");

    let filter = JwtAuthFilter {
        jwks_url,
        issuer: None,
        audience: None,
        header_name: "Authorization".to_string(),
        token_prefix: "Bearer ".to_string(),
        claims_to_headers: vec![],
        cache_ttl_secs: Some(3600),
    };

    let validator = JwtValidator::new(&filter).unwrap();

    let claims = serde_json::json!({"sub": "user-123", "exp": 9999999999_i64});
    let token = keypair.sign_token(&claims);

    let result = validator.validate(&token, &[]).await;
    assert!(
        matches!(result, Err(JwtError::JwksFetchFailed(_))),
        "expected JwksFetchFailed for 404, got: {result:?}"
    );
}

#[tokio::test]
async fn test_empty_claims_to_headers() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/.well-known/jwks.json");

    let filter = make_jwt_auth_filter(&jwks_url);
    let validator = JwtValidator::new(&filter).unwrap();

    let claims =
        serde_json::json!({"sub": "user-789", "email": "test@example.com", "exp": 9999999999_i64});
    let token = keypair.sign_token(&claims);

    let result = validator.validate(&token, &[]).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn test_jwks_cache_hit() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;
    let jwks_url = format!("http://{addr}/.well-known/jwks.json");

    let filter = JwtAuthFilter {
        jwks_url,
        issuer: None,
        audience: None,
        header_name: "Authorization".to_string(),
        token_prefix: "Bearer ".to_string(),
        claims_to_headers: vec![],
        cache_ttl_secs: Some(3600),
    };

    let validator = JwtValidator::new(&filter).unwrap();

    let claims = serde_json::json!({"sub": "user-1", "exp": 9999999999_i64});
    let token = keypair.sign_token(&claims);
    validator.validate(&token, &[]).await.unwrap();

    let claims2 = serde_json::json!({"sub": "user-2", "exp": 9999999999_i64});
    let token2 = keypair.sign_token(&claims2);
    validator.validate(&token2, &[]).await.unwrap();

    assert!(!validator.needs_refresh());
}

#[tokio::test]
async fn test_jwks_empty_keys_response() {
    let app = Router::new().route(
        "/.well-known/jwks.json",
        get(|| async {
            axum::response::Json(serde_json::json!({"keys": [], "exp": 9999999999_i64}))
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        serve(listener, app).await.unwrap();
    });

    let filter = JwtAuthFilter {
        jwks_url: format!("http://{addr}/.well-known/jwks.json"),
        issuer: None,
        audience: None,
        header_name: "Authorization".to_string(),
        token_prefix: "Bearer ".to_string(),
        claims_to_headers: vec![],
        cache_ttl_secs: Some(3600),
    };

    let validator = JwtValidator::new(&filter).unwrap();

    let keypair = TestKeypair::generate();
    let claims = serde_json::json!({"sub": "user-123", "exp": 9999999999_i64});
    let token = keypair.sign_token(&claims);

    let result = validator.validate(&token, &[]).await;
    assert!(
        matches!(result, Err(JwtError::JwksFetchFailed(_))),
        "expected JwksFetchFailed for empty keys, got: {result:?}"
    );
}

#[tokio::test]
async fn test_header_name_customization() {
    let keypair = TestKeypair::generate();
    let addr = start_jwks_server(&keypair).await;

    let filter = JwtAuthFilter {
        jwks_url: format!("http://{addr}/.well-known/jwks.json"),
        issuer: None,
        audience: None,
        header_name: "X-Custom-Auth".to_string(),
        token_prefix: "Token ".to_string(),
        claims_to_headers: vec![],
        cache_ttl_secs: Some(3600),
    };

    let validator = JwtValidator::new(&filter);
    assert!(validator.is_ok());
}

#[test]
fn test_jwt_error_display() {
    assert_eq!(JwtError::MissingToken.to_string(), "missing JWT token");
    assert!(
        JwtError::InvalidToken("bad stuff".to_string())
            .to_string()
            .contains("bad stuff")
    );
    assert!(
        JwtError::NoMatchingKey("kid-123".to_string())
            .to_string()
            .contains("kid-123")
    );
    assert_eq!(JwtError::Expired.to_string(), "JWT token expired");
    assert!(
        JwtError::JwksFetchFailed("timeout".to_string())
            .to_string()
            .contains("timeout")
    );
}
