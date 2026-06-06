use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use getrandom::getrandom;
use hmac::{Hmac, Mac};
use pingora::http::{RequestHeader, ResponseHeader};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::{
    borrow::Cow, collections::BTreeMap, fmt::Write, path::PathBuf, sync::Arc, time::Duration,
};
use tracing::error;

use ntgw_ir::{PersistentSessionTarget, SelectedBackend, SessionPersistence};
use ntgw_observability::ReloadingFile;

type HmacSha256 = Hmac<Sha256>;
const FILE_SECRET_REFRESH_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone)]
pub struct SessionPersistenceOptions {
    secret_source: SecretSource,
}

#[derive(Debug, Clone)]
pub struct ResolvedSession {
    pub target: PersistentSessionTarget,
    absolute_expires_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionToken {
    v: u8,
    sn: Arc<str>,
    st: Arc<str>,
    bn: Arc<str>,
    a: Arc<str>,
    p: u32,
    ax: Option<u64>,
    ix: Option<u64>,
}

#[derive(Clone)]
enum SecretSource {
    Inline(Arc<[u8]>),
    File(FileSecretSource),
    Generated(Arc<[u8]>),
}

#[derive(Clone)]
struct FileSecretSource {
    reloading_secret: ReloadingFile<Arc<[u8]>>,
}

impl SessionPersistenceOptions {
    pub fn build(
        shared_secret: Option<Vec<u8>>,
        shared_secret_file: Option<String>,
    ) -> Result<Self> {
        if let Some(path) = shared_secret_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(Self {
                secret_source: SecretSource::File(FileSecretSource::new(PathBuf::from(path))?),
            });
        }

        if let Some(secret) = shared_secret.filter(|secret| !secret.is_empty()) {
            return Ok(Self {
                secret_source: SecretSource::Inline(Arc::<[u8]>::from(secret)),
            });
        }

        let mut generated = vec![0u8; 32];
        getrandom(&mut generated)?;
        error!(
            "session persistence using auto-generated key; configure sharedSecret or sharedSecretFile for multi-replica deployments"
        );
        Ok(Self {
            secret_source: SecretSource::Generated(Arc::<[u8]>::from(generated)),
        })
    }

    pub fn uses_ephemeral_secret(&self) -> bool {
        matches!(self.secret_source, SecretSource::Generated(_))
    }

    fn shared_secret(&self) -> Result<Arc<[u8]>> {
        match &self.secret_source {
            SecretSource::Inline(secret) | SecretSource::Generated(secret) => Ok(secret.clone()),
            SecretSource::File(source) => source.shared_secret(),
        }
    }
}

#[derive(Clone)]
pub struct SessionManager {
    options: SessionPersistenceOptions,
}

impl SessionManager {
    pub fn new(options: SessionPersistenceOptions) -> Self {
        Self { options }
    }

    pub fn resolve_request_session(
        &self,
        request: &RequestHeader,
        policy: &SessionPersistence,
    ) -> Option<ResolvedSession> {
        let raw_token = if policy.is_cookie() {
            request_cookie(request, &policy.session_name)?
        } else {
            request_header_value(request, &policy.session_name)?
        };

        self.decode_token(policy, &raw_token).ok()
    }

    pub fn resolve_request_session_headers(
        &self,
        headers: &BTreeMap<String, Vec<String>>,
        policy: &SessionPersistence,
    ) -> Option<ResolvedSession> {
        let raw_token = if policy.is_cookie() {
            request_cookie_headers(headers, &policy.session_name)?
        } else {
            request_header_value_headers(headers, &policy.session_name)?
        };

        self.decode_token(policy, &raw_token).ok()
    }

    pub fn write_response_session(
        &self,
        response: &mut ResponseHeader,
        policy: &SessionPersistence,
        selected: &SelectedBackend,
        previous: Option<&ResolvedSession>,
    ) -> pingora::Result<()> {
        let payload = self.build_token_payload(policy, selected, previous);
        let token = self.encode_payload(&payload).map_err(|err| {
            pingora::Error::because(
                pingora::ErrorType::InternalError,
                "failed to encode session token",
                err,
            )
        })?;

        if policy.is_cookie() {
            response.append_header(
                "set-cookie".to_string(),
                build_set_cookie(policy, selected, &token, payload.ax),
            )?;
        } else {
            response.insert_header(policy.session_name.clone(), token)?;
        }

        Ok(())
    }

    fn build_token_payload(
        &self,
        policy: &SessionPersistence,
        selected: &SelectedBackend,
        previous: Option<&ResolvedSession>,
    ) -> SessionToken {
        let now = now_unix();
        let absolute_expires_at = policy.absolute_timeout.map(|timeout| {
            previous
                .and_then(|session| session.absolute_expires_at)
                .unwrap_or_else(|| now.saturating_add(timeout.as_secs()))
        });
        let idle_expires_at = policy
            .idle_timeout
            .map(|timeout| now.saturating_add(timeout.as_secs()));

        SessionToken {
            v: 1,
            sn: Arc::from(policy.session_name.as_str()),
            st: if policy.is_cookie() {
                Arc::from("Cookie")
            } else {
                Arc::from("Header")
            },
            bn: Arc::from(selected.backend_name.as_str()),
            a: Arc::from(selected.backend.address.as_str()),
            p: selected.backend.port,
            ax: absolute_expires_at,
            ix: idle_expires_at,
        }
    }

    fn encode_payload(&self, payload: &SessionToken) -> Result<String> {
        let body = serde_json::to_vec(&payload)?;
        let signature = self.sign(&body)?;
        Ok(format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(body),
            URL_SAFE_NO_PAD.encode(signature)
        ))
    }

    fn decode_token(&self, policy: &SessionPersistence, raw: &str) -> Result<ResolvedSession> {
        let (encoded_body, encoded_signature) = raw
            .split_once('.')
            .ok_or_else(|| anyhow!("malformed session token"))?;
        let body = URL_SAFE_NO_PAD.decode(encoded_body)?;
        let signature = URL_SAFE_NO_PAD.decode(encoded_signature)?;
        self.verify(&body, &signature)?;

        let payload: SessionToken = serde_json::from_slice(&body)?;
        if payload.v != 1 {
            return Err(anyhow!("unsupported session token version"));
        }
        if payload.sn.as_ref() != policy.session_name.as_str() {
            return Err(anyhow!("session token name mismatch"));
        }
        if payload.st.as_ref()
            != if policy.is_cookie() {
                "Cookie"
            } else {
                "Header"
            }
        {
            return Err(anyhow!("session token transport mismatch"));
        }

        let now = now_unix();
        if payload.ax.is_some_and(|expiry| now > expiry) {
            return Err(anyhow!("session token absolute timeout exceeded"));
        }
        if payload.ix.is_some_and(|expiry| now > expiry) {
            return Err(anyhow!("session token idle timeout exceeded"));
        }

        Ok(ResolvedSession {
            target: PersistentSessionTarget {
                backend_name: payload.bn.to_string(),
                endpoint: ntgw_ir::BackendEndpoint {
                    address: payload.a.to_string(),
                    port: payload.p,
                    healthy: true,
                },
            },
            absolute_expires_at: payload.ax,
        })
    }

    fn sign(&self, body: &[u8]) -> Result<Vec<u8>> {
        let secret = self.options.shared_secret()?;
        let mut mac = HmacSha256::new_from_slice(secret.as_ref())?;
        mac.update(body);
        Ok(mac.finalize().into_bytes().to_vec())
    }

    fn verify(&self, body: &[u8], signature: &[u8]) -> Result<()> {
        let secret = self.options.shared_secret()?;
        let mut mac = HmacSha256::new_from_slice(secret.as_ref())?;
        mac.update(body);
        mac.verify_slice(signature)
            .map_err(|_| anyhow!("session token signature mismatch"))
    }
}

impl FileSecretSource {
    fn new(path: PathBuf) -> Result<Self> {
        Self::new_with_refresh_interval(path, FILE_SECRET_REFRESH_INTERVAL)
    }

    fn new_with_refresh_interval(path: PathBuf, refresh_interval: Duration) -> Result<Self> {
        Ok(Self {
            reloading_secret: ReloadingFile::new(
                path,
                refresh_interval,
                parse_session_secret_file,
            )?,
        })
    }

    fn shared_secret(&self) -> Result<Arc<[u8]>> {
        self.reloading_secret.load()
    }
}

fn parse_session_secret_file(bytes: &[u8]) -> Result<Arc<[u8]>> {
    if bytes.is_empty() {
        return Err(anyhow!("session persistence secret file is empty"));
    }

    Ok(Arc::<[u8]>::from(bytes))
}

fn request_header_value(request: &RequestHeader, name: &str) -> Option<String> {
    request
        .headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn request_header_value_headers(
    headers: &BTreeMap<String, Vec<String>>,
    name: &str,
) -> Option<String> {
    headers
        .get(&name.to_ascii_lowercase())
        .and_then(|values| values.first())
        .cloned()
}

fn request_cookie(request: &RequestHeader, name: &str) -> Option<String> {
    for (header_name, value) in request.headers.iter() {
        if !header_name.as_str().eq_ignore_ascii_case("cookie") {
            continue;
        }
        let cookie = value.to_str().ok()?;
        for item in cookie.split(';') {
            if let Some((cookie_name, cookie_value)) = item.trim().split_once('=') {
                if cookie_name == name {
                    return Some(cookie_value.to_string());
                }
            }
        }
    }

    None
}

fn request_cookie_headers(headers: &BTreeMap<String, Vec<String>>, name: &str) -> Option<String> {
    let cookies = headers.get("cookie")?;
    for cookie in cookies {
        for item in cookie.split(';') {
            if let Some((cookie_name, cookie_value)) = item.trim().split_once('=') {
                if cookie_name == name {
                    return Some(cookie_value.to_string());
                }
            }
        }
    }

    None
}

fn build_set_cookie(
    policy: &SessionPersistence,
    selected: &SelectedBackend,
    token: &str,
    absolute_expires_at: Option<u64>,
) -> String {
    let cookie_path = cookie_path(selected);
    let mut cookie = String::with_capacity(256);
    cookie.push_str(&policy.session_name);
    cookie.push('=');
    cookie.push_str(token);
    cookie.push_str("; Path=");
    cookie.push_str(cookie_path.as_ref());
    cookie.push_str("; HttpOnly");

    if policy.cookie_lifetime_type() == "Permanent" {
        if let Some(max_age) = max_age(absolute_expires_at) {
            let _ = write!(&mut cookie, "; Max-Age={max_age}");
        }
    }

    cookie
}

fn max_age(absolute_expires_at: Option<u64>) -> Option<u64> {
    absolute_expires_at.map(|expiry| expiry.saturating_sub(now_unix()).max(1))
}

fn cookie_path(selected: &SelectedBackend) -> Cow<'_, str> {
    let Some(matched_path) = selected.matched_http_path.as_ref() else {
        return Cow::Borrowed("/");
    };

    match matched_path.path_type.as_str() {
        "RegularExpression" => longest_literal_cookie_path(&matched_path.path),
        _ => normalize_cookie_path(&matched_path.path),
    }
}

fn longest_literal_cookie_path(pattern: &str) -> Cow<'_, str> {
    if pattern.is_empty() {
        return Cow::Borrowed("/");
    }

    let mut escaped = false;
    let mut last_slash = 0usize;
    for (index, ch) in pattern.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '.' | '*' | '+' | '?' | '(' | '[' | '{' | '|' | '^' | '$' => break,
            '/' if index > 0 => last_slash = index,
            _ => {}
        }
    }

    if last_slash == 0 {
        return Cow::Borrowed("/");
    }

    normalize_cookie_path(&pattern[..last_slash])
}

fn normalize_cookie_path(path: &str) -> Cow<'_, str> {
    if path.is_empty() || path == "/" {
        return Cow::Borrowed("/");
    }

    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        Cow::Borrowed("/")
    } else if trimmed.starts_with('/') {
        Cow::Owned(trimmed.to_string())
    } else {
        Cow::Owned(format!("/{trimmed}"))
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests;
