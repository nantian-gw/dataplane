use std::{fs, time::Duration};

use anyhow::{anyhow, Result};
use tonic::transport::{Certificate, ClientTlsConfig, Identity};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Default)]
pub struct ConnectOptions {
    pub endpoint: String,
    pub tls: Option<ClientTlsOptions>,
    pub transport: TransportOptions,
}

#[derive(Debug, Clone)]
pub struct TransportOptions {
    pub connect_timeout: Duration,
    pub keepalive_interval: Duration,
    pub keepalive_timeout: Duration,
    pub initial_reconnect_backoff: Duration,
    pub max_reconnect_backoff: Duration,
    pub apply_timeout: Duration,
    pub apply_poll_interval: Duration,
    pub stale_stream_timeout: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct ClientTlsOptions {
    pub ca_path: String,
    pub cert_path: String,
    pub key_path: String,
    pub domain_name: String,
}

pub fn normalize_endpoint(raw: &str, tls_enabled: bool) -> Result<String> {
    let endpoint = raw.trim();
    if endpoint.is_empty() {
        return Err(anyhow!("xds endpoint is required"));
    }

    if let Some(stripped) = endpoint.strip_prefix("http://") {
        if tls_enabled {
            return Ok(format!("https://{stripped}"));
        }
        return Ok(endpoint.to_string());
    }
    if endpoint.starts_with("https://") {
        return Ok(endpoint.to_string());
    }

    let scheme = if tls_enabled { "https" } else { "http" };
    Ok(format!("{scheme}://{endpoint}"))
}

pub fn build_client_tls_config(opts: &ClientTlsOptions) -> Result<ClientTlsConfig> {
    let mut tls = ClientTlsConfig::new();

    if let Some(domain_name) = trim_non_empty(&opts.domain_name) {
        tls = tls.domain_name(domain_name);
    }
    if let Some(ca_path) = trim_non_empty(&opts.ca_path) {
        let pem = fs::read(ca_path)?;
        tls = tls.ca_certificate(Certificate::from_pem(pem));
    }

    let cert_path = trim_non_empty(&opts.cert_path);
    let key_path = trim_non_empty(&opts.key_path);
    match (cert_path, key_path) {
        (Some(cert_path), Some(key_path)) => {
            let cert_pem = fs::read(cert_path)?;
            let key_pem = fs::read(key_path)?;
            tls = tls.identity(Identity::from_pem(cert_pem, key_pem));
        }
        (None, None) => {}
        _ => {
            return Err(anyhow!("xds tls requires both cert_path and key_path"));
        }
    }

    Ok(tls)
}

impl Default for TransportOptions {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            keepalive_interval: Duration::from_secs(10),
            keepalive_timeout: Duration::from_secs(5),
            initial_reconnect_backoff: Duration::from_secs(2),
            max_reconnect_backoff: Duration::from_secs(30),
            apply_timeout: Duration::from_secs(3),
            apply_poll_interval: Duration::from_millis(25),
            stale_stream_timeout: Duration::from_secs(30),
        }
    }
}

fn trim_non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
