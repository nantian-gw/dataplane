use std::{
    borrow::Cow,
    collections::{BTreeMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::mpsc::SyncSender,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::warn;

mod template;
mod writer;

type FlushSyncSender = SyncSender<()>;

#[cfg(test)]
mod tests;

use self::{
    template::render_access_log_template,
    writer::{access_log_writer_snapshot, emit_access_log_line, reset_access_log_writer},
};

const DEFAULT_ROUTE_ANNOTATION_PREFIX: &str = "gateway.nantian.dev/access-log-";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccessLogMode {
    #[default]
    Text,
    Json,
}

impl AccessLogMode {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "json" => Self::Json,
            _ => Self::Text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLogOptions {
    pub enabled: bool,
    pub path: String,
    pub format: String,
    pub mode: AccessLogMode,
    pub sample_rate: f64,
    pub route_annotation_prefix: String,
}

impl Default for AccessLogOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "stdout".to_string(),
            format: "%TIMESTAMP% %EVENT% %PROTOCOL% %LISTENER% %CLIENT_IP% %HOST% %METHOD% %PATH% %REQUEST_ID% %ROUTE_NAMESPACE% %ROUTE_NAME% %BACKEND% %STATUS% %LATENCY_MS% %BYTES_RECEIVED% %BYTES_SENT% %SNAPSHOT_VERSION% %RETRY_ATTEMPTS% %RESPONSE_FLAGS%".to_string(),
            mode: AccessLogMode::Json,
            sample_rate: 1.0,
            route_annotation_prefix: DEFAULT_ROUTE_ANNOTATION_PREFIX.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessLogRecord<'a> {
    pub event: String,
    pub timestamp: String,
    pub start_time_unix_ms: u128,
    pub snapshot_version: String,
    pub listener: Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listener_runtime_id: Option<String>,
    pub protocol: Cow<'a, str>,
    pub client_ip: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub host: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub method: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub request_id: String,
    pub route_namespace: Cow<'a, str>,
    pub route_name: Cow<'a, str>,
    pub route_kind: Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_runtime_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_runtime_id: Option<String>,
    pub backend: Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend_runtime_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_runtime_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    pub latency_ms: u128,
    pub bytes_sent: usize,
    pub bytes_received: usize,
    pub retry_attempts: u32,
    pub response_flags: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub request: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub http_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub query_string: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub referer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user_agent: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub x_forwarded_for: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub upstream_addr: String,
    #[serde(default)]
    pub upstream_connect_time_ms: u128,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub content_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub connection_id: String,
}

impl Default for AccessLogRecord<'static> {
    fn default() -> Self {
        Self {
            event: String::new(),
            timestamp: String::new(),
            start_time_unix_ms: 0,
            snapshot_version: String::new(),
            listener: Cow::Borrowed(""),
            listener_runtime_id: None,
            protocol: Cow::Borrowed(""),
            client_ip: String::new(),
            host: String::new(),
            method: String::new(),
            path: String::new(),
            request_id: String::new(),
            route_namespace: Cow::Borrowed(""),
            route_name: Cow::Borrowed(""),
            route_kind: Cow::Borrowed(""),
            route_runtime_id: None,
            rule_runtime_id: None,
            backend: Cow::Borrowed(""),
            backend_runtime_id: None,
            endpoint_runtime_id: None,
            status: None,
            latency_ms: 0,
            bytes_sent: 0,
            bytes_received: 0,
            retry_attempts: 0,
            response_flags: String::new(),
            request: String::new(),
            http_version: String::new(),
            query_string: String::new(),
            referer: String::new(),
            user_agent: String::new(),
            x_forwarded_for: String::new(),
            upstream_addr: String::new(),
            upstream_connect_time_ms: 0,
            content_type: String::new(),
            connection_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AccessLogSampleKey<'a> {
    pub event: &'a str,
    pub listener: &'a str,
    pub listener_runtime_id: Option<u64>,
    pub request_id: &'a str,
    pub route_namespace: &'a str,
    pub route_name: &'a str,
    pub route_runtime_id: Option<u64>,
    pub backend: &'a str,
    pub backend_runtime_id: Option<u64>,
    pub start_time_unix_ms: u128,
}

impl<'a> AccessLogRecord<'a> {
    pub fn sample_key(&'a self) -> AccessLogSampleKey<'a> {
        AccessLogSampleKey {
            event: self.event.as_str(),
            listener: self.listener.as_ref(),
            listener_runtime_id: parse_runtime_id_hex(self.listener_runtime_id.as_deref()),
            request_id: self.request_id.as_str(),
            route_namespace: self.route_namespace.as_ref(),
            route_name: self.route_name.as_ref(),
            route_runtime_id: parse_runtime_id_hex(self.route_runtime_id.as_deref()),
            backend: self.backend.as_ref(),
            backend_runtime_id: parse_runtime_id_hex(self.backend_runtime_id.as_deref()),
            start_time_unix_ms: self.start_time_unix_ms,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AccessLogWriterSnapshot {
    pub writers: u64,
    pub queue_depth: u64,
    pub dropped_lines_total: u64,
    pub flushes_total: u64,
    pub flush_latency_ms_total: u64,
    pub flush_latency_ms_max: u64,
    pub sink_errors_total: u64,
}

pub fn render_access_log(options: &AccessLogOptions, record: &AccessLogRecord) -> Result<String> {
    match options.mode {
        AccessLogMode::Json => Ok(serde_json::to_string(record)?),
        AccessLogMode::Text => Ok(render_access_log_template(&options.format, record)),
    }
}

pub fn resolve_access_log_options(
    base: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) -> AccessLogOptions {
    resolve_access_log_options_cow(base, route_annotations).into_owned()
}

pub fn access_log_enabled_for_route(
    base: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) -> bool {
    resolve_access_log_options_cow(base, route_annotations).enabled
}

pub fn resolve_access_log_write_options<'a>(
    base: &'a AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
    sample_key: &AccessLogSampleKey<'_>,
) -> Option<Cow<'a, AccessLogOptions>> {
    let resolved = resolve_access_log_options_cow(base, route_annotations);
    (resolved.enabled && should_emit_sample_key(resolved.sample_rate, sample_key))
        .then_some(resolved)
}

pub fn write_access_log(
    base: &AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
    record: &AccessLogRecord,
) -> Result<()> {
    if let Some(resolved) =
        resolve_access_log_write_options(base, route_annotations, &record.sample_key())
    {
        let resolved = resolved.as_ref();
        let line = render_access_log(resolved, record)?;
        emit_access_log(&resolved.path, &line)?;
    }
    Ok(())
}

pub fn emit_access_log(path: &str, line: &str) -> Result<()> {
    emit_access_log_line(path, line.to_string())
}

pub fn snapshot_access_log_writers() -> AccessLogWriterSnapshot {
    access_log_writer_snapshot()
}

pub fn epoch_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default()
}

pub fn current_timestamp() -> String {
    humantime::format_rfc3339_millis(SystemTime::now()).to_string()
}

#[cfg(test)]
fn should_emit_sample(rate: f64, record: &AccessLogRecord) -> bool {
    should_emit_sample_key(rate, &record.sample_key())
}

fn should_emit_sample_key(rate: f64, key: &AccessLogSampleKey<'_>) -> bool {
    if rate <= 0.0 {
        return false;
    }
    if rate >= 1.0 {
        return true;
    }

    let mut hasher = DefaultHasher::new();
    key.event.hash(&mut hasher);
    hash_runtime_id_or_display(&mut hasher, key.listener_runtime_id, key.listener);
    key.request_id.hash(&mut hasher);
    hash_route_sampling_key(&mut hasher, key);
    hash_runtime_id_or_display(&mut hasher, key.backend_runtime_id, key.backend);
    key.start_time_unix_ms.hash(&mut hasher);

    let value = (hasher.finish() % 10_000) as f64 / 10_000.0;
    value < rate
}

fn hash_route_sampling_key(hasher: &mut DefaultHasher, key: &AccessLogSampleKey<'_>) {
    if let Some(runtime_id) = key.route_runtime_id {
        runtime_id.hash(hasher);
    } else {
        key.route_namespace.hash(hasher);
        key.route_name.hash(hasher);
    }
}

fn hash_runtime_id_or_display(hasher: &mut DefaultHasher, runtime_id: Option<u64>, display: &str) {
    if let Some(runtime_id) = runtime_id {
        runtime_id.hash(hasher);
    } else {
        display.hash(hasher);
    }
}

fn resolve_access_log_options_cow<'a>(
    base: &'a AccessLogOptions,
    route_annotations: &BTreeMap<String, String>,
) -> Cow<'a, AccessLogOptions> {
    let prefix = access_log_annotation_prefix(base);
    let mut resolved = None;

    for (key, value) in route_annotations {
        let Some(suffix) = key.strip_prefix(prefix) else {
            continue;
        };

        match suffix {
            "enabled" => match parse_bool(value) {
                Some(enabled) => mutable_access_log_options(base, &mut resolved).enabled = enabled,
                None => {
                    warn!(value = %value, "ignored invalid route access log enabled annotation");
                }
            },
            "path" => {
                // Route annotations may originate from control-plane or tenant input. Do not let
                // them redirect dataplane-local file writes.
            }
            "format" => {
                if let Some(format) = trimmed_non_empty(value) {
                    mutable_access_log_options(base, &mut resolved).format = format.to_string();
                }
            }
            "mode" => {
                mutable_access_log_options(base, &mut resolved).mode = AccessLogMode::parse(value);
            }
            "sample-rate" => match value.trim().parse::<f64>() {
                Ok(rate) if (0.0..=1.0).contains(&rate) => {
                    mutable_access_log_options(base, &mut resolved).sample_rate = rate;
                }
                _ => {
                    warn!(value = %value, "ignored invalid route access log sample-rate annotation");
                }
            },
            _ => {}
        }
    }

    resolved.map(Cow::Owned).unwrap_or(Cow::Borrowed(base))
}

fn mutable_access_log_options<'a>(
    base: &AccessLogOptions,
    resolved: &'a mut Option<AccessLogOptions>,
) -> &'a mut AccessLogOptions {
    resolved.get_or_insert_with(|| base.clone())
}

fn access_log_annotation_prefix(base: &AccessLogOptions) -> &str {
    let prefix = base.route_annotation_prefix.trim();
    if prefix.is_empty() {
        DEFAULT_ROUTE_ANNOTATION_PREFIX
    } else {
        prefix
    }
}

fn parse_runtime_id_hex(value: Option<&str>) -> Option<u64> {
    let value = value?.trim();
    if value.len() != 16 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(value, 16)
        .ok()
        .filter(|parsed| *parsed != 0)
}

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn trimmed_non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn shutdown_access_log_writer(path: &str) {
    reset_access_log_writer(path);
}

pub(crate) fn flush_access_log(path: &str) -> Result<()> {
    writer::flush_access_log(path)
}
