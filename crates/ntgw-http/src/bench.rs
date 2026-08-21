use std::{collections::BTreeMap, time::Duration};

use anyhow::{Context, Result};
use pingora::http::{RequestHeader, ResponseHeader};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

use crate::{
    SessionManager, SessionPersistenceOptions,
    filters::{apply_request_filters, apply_response_filters},
    proxy::{
        RequestContext, RequestView, build_request_meta_from_header_with_port,
        capture_request_context_from_view, start_request_span_from_header,
    },
};
use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendRef, Filter, HeaderModifier, HeaderOperation,
    HttpMatch, HttpRoute, HttpRule, Listener, MatchedHttpPath, RouteKind, SelectedBackend,
    SessionPersistence, Snapshot,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RequestMetaBuildBenchConfig {
    pub header_count: usize,
    pub values_per_header: usize,
    pub query_params: usize,
    pub header_value_bytes: usize,
}

impl Default for RequestMetaBuildBenchConfig {
    fn default() -> Self {
        Self {
            header_count: 96,
            values_per_header: 2,
            query_params: 24,
            header_value_bytes: 48,
        }
    }
}

pub type RequestViewBuildBenchConfig = RequestMetaBuildBenchConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetaBuildStep {
    pub path: String,
    pub header_name_count: usize,
    pub header_value_count: usize,
    pub header_name_bytes: usize,
    pub header_value_bytes: usize,
    pub request_header_bytes: usize,
    pub query_param_count: usize,
    pub query_value_count: usize,
    pub request_id: String,
    pub content_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestViewBuildStep {
    pub host: String,
    pub path: String,
    pub method: String,
    pub client_ip: String,
    pub header_name_count: usize,
    pub header_value_count: usize,
    pub header_name_bytes: usize,
    pub header_value_bytes: usize,
    pub request_header_bytes: usize,
    pub query_param_count: usize,
    pub query_value_count: usize,
    pub request_id: String,
    pub content_length: usize,
}

pub struct RequestMetaBuildFixture {
    request: RequestHeader,
}

impl RequestMetaBuildFixture {
    pub fn build(config: RequestMetaBuildBenchConfig) -> Result<Self> {
        Ok(Self {
            request: build_header_heavy_request(config)?,
        })
    }

    pub fn materialize(&self) -> Result<RequestMetaBuildStep> {
        let meta = build_request_meta_from_header_with_port(&self.request, 8080);
        let header_name_bytes = meta.headers.keys().map(String::len).sum::<usize>();
        let header_value_count = meta.headers.values().map(Vec::len).sum::<usize>();
        let header_value_bytes = meta
            .headers
            .values()
            .flat_map(|values| values.iter())
            .map(String::len)
            .sum::<usize>();
        let request_header_bytes = meta
            .headers
            .iter()
            .map(|(name, values)| {
                values
                    .iter()
                    .map(|value| name.len().saturating_add(value.len()))
                    .sum::<usize>()
            })
            .sum();
        let query_value_count = meta.query_params.values().map(Vec::len).sum::<usize>();
        let request_id = meta
            .headers
            .get("x-request-id")
            .and_then(|values| values.first())
            .cloned()
            .unwrap_or_default();
        let content_length = meta
            .headers
            .get("content-length")
            .and_then(|values| values.first())
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or_default();

        Ok(RequestMetaBuildStep {
            path: meta.path,
            header_name_count: meta.headers.len(),
            header_value_count,
            header_name_bytes,
            header_value_bytes,
            request_header_bytes,
            query_param_count: meta.query_params.len(),
            query_value_count,
            request_id,
            content_length,
        })
    }
}

pub struct RequestViewBuildFixture {
    request: RequestHeader,
    source_ip: String,
}

impl RequestViewBuildFixture {
    pub fn build(config: RequestViewBuildBenchConfig) -> Result<Self> {
        Ok(Self {
            request: build_header_heavy_request(config)?,
            source_ip: "192.0.2.20".to_string(),
        })
    }

    pub fn capture_context(&self) -> RequestContext {
        let view = RequestView::from_header_with_port(&self.request, 8080);
        let mut ctx = RequestContext::default();
        capture_request_context_from_view(&mut ctx, &view, Some(self.source_ip.as_str()));
        start_request_span_from_header(&mut ctx, &self.request);
        ctx
    }

    pub fn capture(&self) -> Result<RequestViewBuildStep> {
        let ctx = self.capture_context();
        let view = RequestView::from_header_with_port(&self.request, 8080);
        let meta = view.materialize();
        let header_name_bytes = meta.headers.keys().map(String::len).sum::<usize>();
        let header_value_count = meta.headers.values().map(Vec::len).sum::<usize>();
        let header_value_bytes = meta
            .headers
            .values()
            .flat_map(|values| values.iter())
            .map(String::len)
            .sum::<usize>();
        let query_value_count = meta.query_params.values().map(Vec::len).sum::<usize>();

        Ok(RequestViewBuildStep {
            host: ctx.host,
            path: ctx.path,
            method: ctx.method,
            client_ip: ctx.client_ip,
            header_name_count: meta.headers.len(),
            header_value_count,
            header_name_bytes,
            header_value_bytes,
            request_header_bytes: view.header_bytes(),
            query_param_count: meta.query_params.len(),
            query_value_count,
            request_id: ctx.request_id,
            content_length: ctx.declared_request_body_bytes,
        })
    }
}

pub struct FastPathSelectionFixture {
    snapshot: Snapshot,
    request: RequestHeader,
}

impl FastPathSelectionFixture {
    pub fn build(config: RequestMetaBuildBenchConfig) -> Result<Self> {
        let mut snapshot = simple_fast_path_snapshot();
        snapshot.rebuild_runtime_indexes();
        Ok(Self {
            snapshot,
            request: build_header_heavy_request(config)?,
        })
    }

    pub fn select(&self) -> Result<ntgw_ir::CompiledSelectedHttpBackend> {
        let request = crate::proxy::fast_path_request_from_header(&self.request, 80);
        self.snapshot
            .select_http_fast_path(request)
            .context("fast path should select benchmark backend")
    }
}

fn simple_fast_path_snapshot() -> Snapshot {
    Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            port: 80,
            protocol: "HTTP".to_string(),
            attached_routes: vec!["default/bench".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "bench".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["bench.example.com".to_string()],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/bench".to_string(),
                    path_type: "PathPrefix".to_string(),
                    method: "GET".to_string(),
                    ..HttpMatch::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "bench".to_string(),
                    port: 8080,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            name: "bench:8080".into(),
            namespace: "default".into(),
            protocol: "HTTP".into(),
            wasm_plugin: None,
            ai_service: None,
            token_policy: None,
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],

            circuit_breaker: None,
            security_policy: None,
        }],
        ..Snapshot::default()
    }
}

fn build_header_heavy_request(config: RequestMetaBuildBenchConfig) -> Result<RequestHeader> {
    let uri = header_heavy_uri(config.query_params.max(1));
    let mut request = RequestHeader::build("GET", uri.as_bytes(), None)
        .context("building header-heavy benchmark request")?;
    request
        .insert_header("host", "bench.example.com")
        .context("adding benchmark host header")?;
    request
        .insert_header("x-request-id", "bench-request-id")
        .context("adding benchmark request id header")?;
    request
        .insert_header("content-length", "1234")
        .context("adding benchmark content length header")?;
    request
        .insert_header(
            "traceparent",
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        )
        .context("adding benchmark traceparent header")?;

    let value = padded_header_value(config.header_value_bytes.max(1));
    for header_index in 0..config.header_count.max(1) {
        let name = format!("x-bench-header-{header_index:03}");
        for value_index in 0..config.values_per_header.max(1) {
            request
                .append_header(name.clone(), format!("{value}-{value_index:02}"))
                .context("adding repeated benchmark request header")?;
        }
    }

    Ok(request)
}

fn header_heavy_uri(query_params: usize) -> String {
    let mut uri = String::from("/bench/header-heavy?");
    for index in 0..query_params {
        if index > 0 {
            uri.push('&');
        }
        uri.push_str(&format!("param{index}=value{index}"));
    }
    uri
}

fn padded_header_value(bytes: usize) -> String {
    "v".repeat(bytes)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FilterChainBenchConfig {
    pub request_filters: usize,
    pub response_filters: usize,
    pub header_ops_per_filter: usize,
}

impl Default for FilterChainBenchConfig {
    fn default() -> Self {
        Self {
            request_filters: 8,
            response_filters: 8,
            header_ops_per_filter: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterChainStep {
    pub request_header_count: usize,
    pub response_header_count: usize,
    pub request_marker: String,
    pub response_marker: String,
}

pub struct FilterChainFixture {
    request_filters: Vec<Filter>,
    response_filters: Vec<Filter>,
    matched_http_path: MatchedHttpPath,
}

impl FilterChainFixture {
    pub fn build(config: FilterChainBenchConfig) -> Self {
        let request_filters = (0..config.request_filters.max(1))
            .map(|filter_index| Filter {
                filter_type: "RequestHeaderModifier".to_string(),
                header_modifier: Some(HeaderModifier {
                    set: (0..config.header_ops_per_filter.max(1))
                        .map(|op_index| HeaderOperation {
                            name: format!("x-request-set-{filter_index}-{op_index}"),
                            value: format!("set-{filter_index}-{op_index}"),
                        })
                        .collect(),
                    add: (0..config.header_ops_per_filter.max(1))
                        .map(|op_index| HeaderOperation {
                            name: format!("x-request-add-{filter_index}-{op_index}"),
                            value: format!("add-{filter_index}-{op_index}"),
                        })
                        .collect(),
                    remove: (0..config.header_ops_per_filter.max(1))
                        .map(|op_index| format!("x-request-remove-{filter_index}-{op_index}"))
                        .collect(),
                }),
                ..Filter::default()
            })
            .collect();
        let response_filters = (0..config.response_filters.max(1))
            .map(|filter_index| Filter {
                filter_type: "ResponseHeaderModifier".to_string(),
                header_modifier: Some(HeaderModifier {
                    set: (0..config.header_ops_per_filter.max(1))
                        .map(|op_index| HeaderOperation {
                            name: format!("x-response-set-{filter_index}-{op_index}"),
                            value: format!("set-{filter_index}-{op_index}"),
                        })
                        .collect(),
                    add: (0..config.header_ops_per_filter.max(1))
                        .map(|op_index| HeaderOperation {
                            name: format!("x-response-add-{filter_index}-{op_index}"),
                            value: format!("add-{filter_index}-{op_index}"),
                        })
                        .collect(),
                    remove: (0..config.header_ops_per_filter.max(1))
                        .map(|op_index| format!("x-response-remove-{filter_index}-{op_index}"))
                        .collect(),
                }),
                ..Filter::default()
            })
            .collect();

        Self {
            request_filters,
            response_filters,
            matched_http_path: MatchedHttpPath {
                path: "/bench".to_string(),
                path_type: "PathPrefix".to_string(),
            },
        }
    }

    pub fn apply(&self) -> Result<FilterChainStep> {
        let mut request = RequestHeader::build("GET", b"/bench/items?id=1", None)
            .context("building benchmark request header")?;
        request
            .insert_header("host", "bench.example.com")
            .context("adding benchmark host header")?;
        for filter_index in 0..self.request_filters.len() {
            for op_index in 0..self.request_filters[filter_index]
                .header_modifier
                .as_ref()
                .map(|modifier| modifier.remove.len())
                .unwrap_or_default()
            {
                request
                    .insert_header(
                        format!("x-request-remove-{filter_index}-{op_index}"),
                        "drop-me",
                    )
                    .context("adding removable request benchmark header")?;
            }
        }
        apply_request_filters(
            &mut request,
            &self.request_filters,
            Some(&self.matched_http_path),
        )
        .context("applying request header benchmark filters")?;

        let mut response =
            ResponseHeader::build(200, None).context("building benchmark response header")?;
        for filter_index in 0..self.response_filters.len() {
            for op_index in 0..self.response_filters[filter_index]
                .header_modifier
                .as_ref()
                .map(|modifier| modifier.remove.len())
                .unwrap_or_default()
            {
                response
                    .insert_header(
                        format!("x-response-remove-{filter_index}-{op_index}"),
                        "drop-me",
                    )
                    .context("adding removable response benchmark header")?;
            }
        }
        apply_response_filters(&mut response, &self.response_filters, None, None)
            .context("applying response header benchmark filters")?;

        Ok(FilterChainStep {
            request_header_count: request.headers.len(),
            response_header_count: response.headers.len(),
            request_marker: header_value(
                request.headers.get("x-request-set-0-0"),
                "request marker",
            )?,
            response_marker: header_value(
                response.headers.get("x-response-set-0-0"),
                "response marker",
            )?,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SessionBenchConfig {
    pub absolute_timeout_secs: u64,
    pub idle_timeout_secs: u64,
}

impl Default for SessionBenchConfig {
    fn default() -> Self {
        Self {
            absolute_timeout_secs: 300,
            idle_timeout_secs: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionBenchStep {
    pub backend_name: String,
    pub endpoint_address: String,
    pub endpoint_port: u32,
    pub token_len: usize,
}

pub struct SessionBenchFixture {
    manager: SessionManager,
    policy: SessionPersistence,
    selected: SelectedBackend,
}

impl SessionBenchFixture {
    pub fn build(config: SessionBenchConfig) -> Result<Self> {
        let manager = SessionManager::new(
            SessionPersistenceOptions::build(
                Some(b"0123456789abcdef0123456789abcdef".to_vec()),
                None,
            )
            .context("building benchmark session secret")?,
        );
        Ok(Self {
            manager,
            policy: SessionPersistence {
                session_name: "ntgw-bench-session".to_string(),
                session_type: "Cookie".to_string(),
                absolute_timeout: Some(Duration::from_secs(config.absolute_timeout_secs)),
                idle_timeout: Some(Duration::from_secs(config.idle_timeout_secs)),
                cookie: Some(ntgw_ir::CookieConfig {
                    lifetime_type: "Permanent".to_string(),
                }),
            },
            selected: SelectedBackend {
                route_policy: None,
                route_kind: RouteKind::Http,
                route_name: "bench-route".to_string(),
                route_namespace: "default".to_string(),
                rule_index: None,
                route_annotations: BTreeMap::new(),
                listener_name: "default/gw/http".to_string(),
                listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                backend: BackendEndpoint {
                    address: "10.20.0.42".to_string(),
                    port: 8080,
                    healthy: true,
                },
                backend_name: "default/bench-backend:8080".to_string(),
                filters: Vec::new(),
                matched_http_path: Some(MatchedHttpPath {
                    path: "/bench".to_string(),
                    path_type: "PathPrefix".to_string(),
                }),
                timeouts: None,
                retry: None,
                session_persistence: None,
                backend_tls: None,
            },
        })
    }

    pub fn encode_decode_cycle(&self) -> Result<SessionBenchStep> {
        let mut response =
            ResponseHeader::build(200, None).context("building session benchmark response")?;
        self.manager
            .write_response_session(&mut response, &self.policy, &self.selected, None)
            .context("encoding session benchmark token")?;

        let token = response
            .headers
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .and_then(|value| value.split_once('='))
            .map(|(_, value)| value.to_string())
            .context("extracting session benchmark cookie")?;

        let mut request =
            RequestHeader::build("GET", b"/bench", None).context("building session request")?;
        request
            .insert_header("cookie", format!("{}={token}", self.policy.session_name))
            .context("adding session benchmark cookie")?;
        let resolved = self
            .manager
            .resolve_request_session(&request, &self.policy)
            .context("decoding session benchmark token")?;

        Ok(SessionBenchStep {
            backend_name: resolved.target.backend_name,
            endpoint_address: resolved.target.endpoint.address,
            endpoint_port: resolved.target.endpoint.port,
            token_len: token.len(),
        })
    }
}

fn header_value(value: Option<&http::HeaderValue>, field_name: &str) -> Result<String> {
    value
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .with_context(|| format!("missing benchmark header value for {field_name}"))
}

// --- Backend TLS cache key construction benchmark ---

use std::hash::{Hash, Hasher};

use ntgw_ir::{BackendSubjectAltName, BackendTlsValidation};

use crate::proxy::cache::BackendTlsValidationCacheKey;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TlsCacheKeyBenchConfig {
    pub ca_pem_count: usize,
    pub ca_pem_bytes: usize,
    pub san_count: usize,
}

impl Default for TlsCacheKeyBenchConfig {
    fn default() -> Self {
        Self {
            ca_pem_count: 3,
            ca_pem_bytes: 2048,
            san_count: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsCacheKeyStep {
    pub cache_key_hash: u64,
    pub ca_pem_total_bytes: usize,
    pub san_count: usize,
}

pub struct TlsCacheKeyFixture {
    validation: BackendTlsValidation,
}

impl TlsCacheKeyFixture {
    pub fn build(config: TlsCacheKeyBenchConfig) -> Self {
        let ca_pems: Vec<String> = (0..config.ca_pem_count)
            .map(|i| {
                format!(
                    "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----",
                    "A".repeat(config.ca_pem_bytes)
                        .replace(&"A".repeat(8), &format!("{i:08}"))
                )
            })
            .collect();
        let subject_alt_names: Vec<BackendSubjectAltName> = (0..config.san_count)
            .map(|i| BackendSubjectAltName {
                kind: "DNS".to_string(),
                value: format!("backend-{i}.bench.internal"),
            })
            .collect();
        let validation = BackendTlsValidation {
            hostname: "backend-0.bench.internal".to_string(),
            use_system_ca_certificates: true,
            ca_pems,
            subject_alt_names,
            min_version: "1.2".to_string(),
            max_version: "1.3".to_string(),
        };
        Self { validation }
    }

    pub fn construct_key(&self) -> TlsCacheKeyStep {
        let key = BackendTlsValidationCacheKey::new(&self.validation);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        TlsCacheKeyStep {
            cache_key_hash: hasher.finish(),
            ca_pem_total_bytes: self.validation.ca_pems.iter().map(|p| p.len()).sum(),
            san_count: self.validation.subject_alt_names.len(),
        }
    }
}
