use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::{Result, anyhow};
use axum::{
    extract::State,
    http::{HeaderValue, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use ntgw_observability::ReloadingFile;
use subtle::ConstantTimeEq;

use super::AdminRuntimeConfig;

const ADMIN_BEARER_TOKEN_REFRESH_INTERVAL: Duration = Duration::ZERO;

#[derive(Clone)]
pub(crate) struct AdminAuth {
    config: Arc<RwLock<AdminRuntimeConfig>>,
}

impl AdminAuth {
    pub(crate) fn new(config: Arc<RwLock<AdminRuntimeConfig>>) -> Self {
        Self { config }
    }

    fn resolve_bearer_token(&self) -> Option<Arc<str>> {
        let config = self
            .config
            .read()
            .unwrap_or_else(|err| err.into_inner())
            .clone();

        let bearer_token_file = config
            .admin_bearer_token_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                ReloadingFile::new_lazy(
                    PathBuf::from(value),
                    ADMIN_BEARER_TOKEN_REFRESH_INTERVAL,
                    parse_admin_bearer_token,
                )
            });
        if let Some(token_file) = bearer_token_file.as_ref() {
            return token_file.load().ok();
        }

        config
            .admin_bearer_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(Arc::<str>::from)
    }
}

pub(crate) async fn require_bearer_auth(
    State(auth): State<AdminAuth>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let Some(expected) = auth.resolve_bearer_token() else {
        return next.run(request).await;
    };
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| is_authorized(value, expected.as_ref()));

    if authorized {
        next.run(request).await
    } else {
        unauthorized_response()
    }
}

pub(crate) fn is_authorized(value: &str, expected: &str) -> bool {
    let mut parts = value.split_whitespace();
    let Some(scheme) = parts.next() else {
        return false;
    };
    let Some(token) = parts.next() else {
        return false;
    };
    if parts.next().is_some() || !scheme.eq_ignore_ascii_case("Bearer") {
        return false;
    }

    token.as_bytes().ct_eq(expected.as_bytes()).into()
}

fn unauthorized_response() -> Response {
    let mut response = StatusCode::UNAUTHORIZED.into_response();
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Bearer realm=\"nantian-dataplane-admin\""),
    );
    response
}

fn parse_admin_bearer_token(bytes: &[u8]) -> Result<Arc<str>> {
    let raw = std::str::from_utf8(bytes)?;
    let token = raw.trim();
    if token.is_empty() {
        return Err(anyhow!("admin bearer token file is empty"));
    }
    Ok(Arc::<str>::from(token))
}
