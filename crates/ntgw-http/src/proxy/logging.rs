use std::borrow::Cow;

use ntgw_observability::{
    AccessLogOptions, AccessLogRecord, AccessLogSampleKey, SharedTrafficStats,
    TrafficObservationRef, TrafficRuntimeIds, current_timestamp, emit_access_log,
    render_access_log, resolve_access_log_write_options,
};
use tracing::error;

use super::context::{
    RequestContext, clear_completed_request_context, route_kind_name, saturating_latency_ms,
};
use super::request::access_log_route_annotations;
use super::retry::retry_completed_successfully;

fn extract_request_header<'a>(ctx: &'a RequestContext, name: &str) -> Cow<'a, str> {
    ctx.access_log_request_headers
        .get(name)
        .map(|s| s.as_str())
        .or_else(|| {
            ctx.request_headers
                .as_ref()
                .and_then(|headers| headers.get(name))
                .and_then(|values| values.first())
                .map(|s| s.as_str())
        })
        .filter(|s| !s.is_empty())
        .map(Cow::Borrowed)
        .unwrap_or(Cow::Borrowed("-"))
}

fn build_request_line(ctx: &RequestContext) -> String {
    let path_and_query = if ctx.query_string.is_empty() {
        ctx.path.clone()
    } else {
        format!("{}?{}", ctx.path, ctx.query_string)
    };
    let version = if ctx.http_version.is_empty() {
        String::from("HTTP/1.1")
    } else {
        ctx.http_version.clone()
    };
    format!("{} {} {}", ctx.method, path_and_query, version)
}

pub(crate) fn observe_completed_request(
    access_log: &AccessLogOptions,
    traffic: &SharedTrafficStats,
    ctx: &mut RequestContext,
    latency_ms: u128,
    bytes_sent: usize,
) {
    if !ctx.is_mirror_subrequest {
        let route_labels = request_route_labels(ctx);
        let traffic_topology = ctx
            .selected_backend_config
            .as_ref()
            .map(|config| config.traffic_topology.as_ref())
            .or_else(|| {
                ctx.local_response_traffic_topology
                    .as_deref()
                    .map(|topology| topology.as_ref())
            });
        traffic.observe_ref_with_topology(
            TrafficObservationRef {
                listener_name: route_labels.listener_name,
                protocol: route_labels.effective_protocol(),
                route_namespace: route_labels.route_namespace,
                route_name: route_labels.route_name,
                route_kind: route_labels.route_kind,
                backend_name: route_labels.backend_name,
                status: Some(ctx.status),
                latency_ms: saturating_latency_ms(latency_ms),
                bytes_received: ctx.bytes_received as u64,
                bytes_sent: bytes_sent as u64,
                retry_attempts: ctx.retry_attempts,
                retried_success: retry_completed_successfully(ctx),
                upstream_pool_hits: ctx.upstream_pool_hits,
                upstream_pool_misses: ctx.upstream_pool_misses,
                upstream_peer_build_failures: ctx.upstream_peer_build_failures,
                upstream_connect_latency_ms: ctx.upstream_connect_latency_ms,
                upstream_connect_latency_ms_max: ctx.upstream_connect_latency_ms_max,
                upstream_connect_latency_ms_buckets: &ctx.upstream_connect_latency_ms_buckets,
                response_flags: ctx.response_flags.as_str(),
                runtime_ids: TrafficRuntimeIds {
                    listener: ctx.runtime_ids.listener.map(|id| id.as_u64()),
                    route: ctx.runtime_ids.route.map(|id| id.as_u64()),
                    backend: ctx.runtime_ids.backend.map(|id| id.as_u64()),
                },
            },
            traffic_topology,
        );
    }

    if !access_log.enabled || ctx.is_mirror_subrequest {
        clear_completed_request_context(ctx);
        return;
    }

    let route_annotations = access_log_route_annotations(ctx);
    let sample_key = access_log_sample_key(ctx);
    let Some(resolved_access_log) =
        resolve_access_log_write_options(access_log, route_annotations, &sample_key)
    else {
        clear_completed_request_context(ctx);
        return;
    };

    let write_result = {
        let route_labels = request_route_labels(ctx);
        let record = AccessLogRecord {
            event: "http_request".to_string(),
            timestamp: current_timestamp(),
            start_time_unix_ms: ctx.started_at_unix_ms,
            snapshot_version: ctx.snapshot_version.clone(),
            listener: Cow::Borrowed(route_labels.listener_name),
            listener_runtime_id: ctx.runtime_ids.listener.map(|id| id.to_string()),
            protocol: Cow::Borrowed(route_labels.effective_protocol()),
            client_ip: ctx.client_ip.clone(),
            host: ctx.host.clone(),
            method: ctx.method.clone(),
            path: ctx.path.clone(),
            request_id: ctx.request_id.clone(),
            route_namespace: Cow::Borrowed(route_labels.route_namespace),
            route_name: Cow::Borrowed(route_labels.route_name),
            route_kind: Cow::Borrowed(route_labels.route_kind),
            route_runtime_id: ctx.runtime_ids.route.map(|id| id.to_string()),
            rule_runtime_id: ctx.runtime_ids.rule.map(|id| id.to_string()),
            backend: Cow::Borrowed(route_labels.backend_name),
            backend_runtime_id: ctx.runtime_ids.backend.map(|id| id.to_string()),
            endpoint_runtime_id: ctx.runtime_ids.endpoint.map(|id| id.to_string()),
            status: Some(ctx.status),
            latency_ms,
            bytes_sent,
            bytes_received: ctx.bytes_received,
            retry_attempts: ctx.retry_attempts,
            response_flags: ctx.response_flags.clone(),
            request: build_request_line(ctx),
            http_version: ctx.http_version.clone(),
            query_string: ctx.query_string.clone(),
            referer: extract_request_header(ctx, "referer").into_owned(),
            user_agent: extract_request_header(ctx, "user-agent").into_owned(),
            x_forwarded_for: extract_request_header(ctx, "x-forwarded-for").into_owned(),
            upstream_addr: ctx.upstream_addr.clone(),
            upstream_connect_time_ms: ctx.upstream_connect_latency_ms as u128,
            content_type: ctx.response_content_type.clone(),
            connection_id: ctx.connection_id.clone(),
            request_header_values: ctx.access_log_request_headers.clone(),
            sent_response_header_values: ctx.access_log_sent_response_headers.clone(),
            upstream_response_header_values: ctx.access_log_upstream_response_headers.clone(),
            upstream_statuses: ctx.access_log_upstream_statuses.clone(),
            scheme: ctx.access_log_scheme.clone(),
            remote_port: ctx.access_log_remote_port,
        };
        render_access_log(&resolved_access_log, &record)
            .and_then(|line| emit_access_log(&resolved_access_log.path, &line))
    };
    if let Err(err) = write_result {
        error!(error = %err, "failed to emit access log");
    }

    clear_completed_request_context(ctx);
}

fn access_log_sample_key(ctx: &RequestContext) -> AccessLogSampleKey<'_> {
    let route_labels = request_route_labels(ctx);
    AccessLogSampleKey {
        event: "http_request",
        listener: route_labels.listener_name,
        listener_runtime_id: ctx.runtime_ids.listener.map(|id| id.as_u64()),
        request_id: ctx.request_id.as_str(),
        route_namespace: route_labels.route_namespace,
        route_name: route_labels.route_name,
        route_runtime_id: ctx.runtime_ids.route.map(|id| id.as_u64()),
        backend: route_labels.backend_name,
        backend_runtime_id: ctx.runtime_ids.backend.map(|id| id.as_u64()),
        start_time_unix_ms: ctx.started_at_unix_ms,
    }
}

#[derive(Clone, Copy)]
struct RequestRouteLabels<'a> {
    listener_name: &'a str,
    listener_protocol: &'a str,
    route_namespace: &'a str,
    route_name: &'a str,
    route_kind: &'a str,
    backend_name: &'a str,
}

impl<'a> RequestRouteLabels<'a> {
    fn effective_protocol(&self) -> &'a str {
        if !self.listener_protocol.is_empty() {
            return self.listener_protocol;
        }

        if self.route_kind.eq_ignore_ascii_case("grpc") {
            "GRPC"
        } else {
            "HTTP"
        }
    }
}

fn request_route_labels(ctx: &RequestContext) -> RequestRouteLabels<'_> {
    if let Some(selected) = ctx.selected_backend.as_ref() {
        return RequestRouteLabels {
            listener_name: selected.listener_name.as_ref(),
            listener_protocol: selected.listener_protocol.as_ref(),
            route_namespace: selected.route_namespace.as_ref(),
            route_name: selected.route_name.as_ref(),
            route_kind: route_kind_name(&selected.route_kind),
            backend_name: selected.backend_name.as_ref(),
        };
    }

    if let Some(selected) = ctx
        .fast_selected_backend
        .as_ref()
        .map(|state| &state.selected)
    {
        return RequestRouteLabels {
            listener_name: selected.listener_name.as_ref(),
            listener_protocol: selected.listener_protocol.as_ref(),
            route_namespace: selected.route_namespace.as_ref(),
            route_name: selected.route_name.as_ref(),
            route_kind: route_kind_name(&selected.route_kind),
            backend_name: selected.backend_name.as_ref(),
        };
    }

    RequestRouteLabels {
        listener_name: ctx.listener_name.as_str(),
        listener_protocol: ctx.listener_protocol.as_str(),
        route_namespace: ctx.route_namespace.as_str(),
        route_name: ctx.route_name.as_str(),
        route_kind: ctx.route_kind.as_str(),
        backend_name: ctx.backend.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        path::Path,
        sync::Arc,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use crate::proxy::context::cache_fast_selected_backend_state;
    use ntgw_ir::{
        BackendCluster, BackendEndpoint, BackendRef, CompiledSelectedHttpBackend, HttpRoute,
        HttpRule, Listener, RequestMeta, RouteKind, SelectedBackend, SelectedBackendRuntimeIds,
        Snapshot,
    };
    use ntgw_observability::shutdown_access_log_writer;

    use super::*;

    #[test]
    fn route_annotations_for_log_prefers_selected_backend_annotations() {
        let ctx = RequestContext {
            route_annotations: BTreeMap::from([("stale".to_string(), "1".to_string())]),
            selected_backend: Some(Arc::new(SelectedBackend {
                route_policy: None,
                route_kind: RouteKind::Http,
                route_name: "route".to_string(),
                route_namespace: "default".to_string(),
                rule_index: None,
                route_annotations: BTreeMap::from([(
                    "gateway.nantian.dev/access-log-mode".to_string(),
                    "json".to_string(),
                )]),
                listener_name: "default/gw/http".to_string(),
                listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                backend: BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: 8080,
                    healthy: true,
                },
                backend_name: "default/echo:8080".to_string(),
                filters: Vec::new(),
                matched_http_path: None,
                timeouts: None,
                retry: None,
                session_persistence: None,
                backend_tls: None,
            })),
            ..RequestContext::default()
        };

        let annotations = access_log_route_annotations(&ctx);
        assert_eq!(
            annotations
                .get("gateway.nantian.dev/access-log-mode")
                .map(String::as_str),
            Some("json")
        );
        assert!(!annotations.contains_key("stale"));
    }

    #[test]
    fn route_annotations_for_log_prefers_fast_selected_backend_annotations() {
        let selected = CompiledSelectedHttpBackend {
            route_kind: RouteKind::Http,
            route_name: "route".into(),
            route_namespace: "default".into(),
            rule_index: None,
            route_annotations: Arc::new(BTreeMap::from([(
                "gateway.nantian.dev/access-log-sample-rate".to_string(),
                "0".to_string(),
            )])),
            listener_name: "default/gw/http".into(),
            listener_protocol: "HTTP".into(),
            backend: BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: 8080,
                healthy: true,
            },
            backend_name: "default/echo:8080".into(),
            matched_http_path: ntgw_ir::MatchedHttpPath::default(),
            runtime_ids: SelectedBackendRuntimeIds::default(),
        };
        let mut ctx = RequestContext {
            route_annotations: BTreeMap::from([("stale".to_string(), "1".to_string())]),
            ..RequestContext::default()
        };
        cache_fast_selected_backend_state(&mut ctx, selected, true);

        let annotations = access_log_route_annotations(&ctx);
        assert_eq!(
            annotations
                .get("gateway.nantian.dev/access-log-sample-rate")
                .map(String::as_str),
            Some("0")
        );
        assert!(!annotations.contains_key("stale"));
    }

    #[test]
    fn route_annotations_for_log_uses_context_annotations_without_selected_backend() {
        let ctx = RequestContext {
            route_annotations: BTreeMap::from([(
                "gateway.nantian.dev/access-log-mode".to_string(),
                "json".to_string(),
            )]),
            ..RequestContext::default()
        };

        assert_eq!(
            access_log_route_annotations(&ctx)
                .get("gateway.nantian.dev/access-log-mode")
                .map(String::as_str),
            Some("json")
        );
    }

    #[test]
    fn observe_completed_request_renders_nginx_style_http_variables() {
        let log_path = temp_log_path("nginx-style-http");
        let path_text = log_path.display().to_string();
        let traffic = SharedTrafficStats::shared();
        let mut ctx = RequestContext {
            started_at_unix_ms: 123,
            snapshot_version: "v1".to_string(),
            client_ip: "192.0.2.10".to_string(),
            host: "orders.example.com".to_string(),
            method: "GET".to_string(),
            path: "/orders".to_string(),
            query_string: "id=1".to_string(),
            request_id: "req-1".to_string(),
            status: 200,
            listener_name: "default/gw/http".to_string(),
            listener_protocol: "HTTP".to_string(),
            route_name: "orders".to_string(),
            route_namespace: "default".to_string(),
            route_kind: "HTTPRoute".to_string(),
            backend: "default/orders:8080".to_string(),
            http_version: "HTTP/2".to_string(),
            upstream_addr: "10.0.0.10:8080".to_string(),
            access_log_request_headers: BTreeMap::from([(
                "user-agent".to_string(),
                "curl/8.7.1".to_string(),
            )]),
            ..RequestContext::default()
        };

        observe_completed_request(
            &AccessLogOptions {
                enabled: true,
                path: path_text.clone(),
                mode: ntgw_observability::AccessLogMode::Text,
                format: r#"$remote_addr "$request" $status $request_time "$http_user_agent""#
                    .to_string(),
                ..AccessLogOptions::default()
            },
            &traffic,
            &mut ctx,
            123,
            128,
        );

        let contents = wait_for_log_contents(&log_path);
        assert!(
            contents.contains(r#"192.0.2.10 "GET /orders?id=1 HTTP/2" 200 0.123 "curl/8.7.1""#)
        );

        shutdown_access_log_writer(&path_text);
        let _ = fs::remove_file(log_path);
    }

    #[test]
    fn observe_completed_request_renders_response_side_nginx_variables() {
        let log_path = temp_log_path("nginx-style-response-vars");
        let path_text = log_path.display().to_string();
        let traffic = SharedTrafficStats::shared();
        let mut ctx = RequestContext {
            started_at_unix_ms: 123,
            client_ip: "192.0.2.10".to_string(),
            host: "orders.example.com".to_string(),
            method: "GET".to_string(),
            path: "/orders".to_string(),
            request_id: "req-1".to_string(),
            status: 200,
            listener_name: "default/gw/http".to_string(),
            listener_protocol: "HTTP".to_string(),
            route_name: "orders".to_string(),
            route_namespace: "default".to_string(),
            route_kind: "HTTPRoute".to_string(),
            backend: "default/orders:8080".to_string(),
            access_log_scheme: "https".to_string(),
            access_log_remote_port: Some(54432),
            access_log_sent_response_headers: BTreeMap::from([(
                "content-type".to_string(),
                "application/json".to_string(),
            )]),
            access_log_upstream_response_headers: BTreeMap::from([(
                "server".to_string(),
                "orders-upstream".to_string(),
            )]),
            access_log_upstream_statuses: vec![502, 200],
            ..RequestContext::default()
        };

        observe_completed_request(
            &AccessLogOptions {
                enabled: true,
                path: path_text.clone(),
                mode: ntgw_observability::AccessLogMode::Text,
                format: r#"$scheme $remote_port "$sent_http_content_type" "$upstream_http_server" $upstream_status"#.to_string(),
                ..AccessLogOptions::default()
            },
            &traffic,
            &mut ctx,
            123,
            128,
        );

        let contents = wait_for_log_contents(&log_path);
        assert!(contents.contains(r#"https 54432 "application/json" "orders-upstream" 502, 200"#));

        shutdown_access_log_writer(&path_text);
        let _ = fs::remove_file(log_path);
    }

    #[test]
    fn observe_completed_request_honors_nginx_style_route_override_format() {
        let log_path = temp_log_path("nginx-style-override");
        let path_text = log_path.display().to_string();
        let traffic = SharedTrafficStats::shared();
        let mut ctx = RequestContext {
            started_at_unix_ms: 123,
            snapshot_version: "v1".to_string(),
            client_ip: "192.0.2.10".to_string(),
            host: "orders.example.com".to_string(),
            method: "GET".to_string(),
            path: "/orders".to_string(),
            query_string: "id=1".to_string(),
            request_id: "req-1".to_string(),
            status: 200,
            listener_name: "default/gw/http".to_string(),
            listener_protocol: "HTTP".to_string(),
            route_name: "orders".to_string(),
            route_namespace: "default".to_string(),
            route_kind: "HTTPRoute".to_string(),
            backend: "default/orders:8080".to_string(),
            route_annotations: BTreeMap::from([
                (
                    "gateway.nantian.dev/access-log-mode".to_string(),
                    "text".to_string(),
                ),
                (
                    "gateway.nantian.dev/access-log-format".to_string(),
                    "$remote_addr $ntgw_route_name".to_string(),
                ),
            ]),
            ..RequestContext::default()
        };

        observe_completed_request(
            &AccessLogOptions {
                enabled: true,
                path: path_text.clone(),
                mode: ntgw_observability::AccessLogMode::Json,
                ..AccessLogOptions::default()
            },
            &traffic,
            &mut ctx,
            123,
            128,
        );

        let contents = wait_for_log_contents(&log_path);
        assert!(contents.contains("192.0.2.10 orders"));

        shutdown_access_log_writer(&path_text);
        let _ = fs::remove_file(log_path);
    }

    #[test]
    fn observe_completed_request_emits_runtime_ids_in_access_log() {
        let mut snapshot = Snapshot {
            listeners: vec![Listener {
                name: "default/gw/http".to_string(),
                address: "0.0.0.0".to_string(),
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                attached_routes: vec!["default/orders".to_string()],
                ..Listener::default()
            }],
            http_routes: vec![HttpRoute {
                name: "orders".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["orders.example.com".to_string()],
                rules: vec![HttpRule {
                    name: String::new(),
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "orders".to_string(),
                        port: 8080,
                        ..BackendRef::default()
                    }],
                    ..HttpRule::default()
                }],
                ..HttpRoute::default()
            }],
            backends: vec![BackendCluster {
                ai_service: None,
                token_policy: None,
                name: "orders:8080".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
            }],
            ..Snapshot::default()
        };
        snapshot.rebuild_runtime_indexes();
        let selected = snapshot
            .select_backend(&RequestMeta::new(
                Some("orders.example.com".to_string()),
                "/",
                "GET",
                BTreeMap::new(),
            ))
            .expect("selected backend");
        let runtime_ids = snapshot.selected_backend_runtime_ids(&selected);
        let expected_listener = runtime_ids.listener.expect("listener id").to_string();
        let expected_route = runtime_ids.route.expect("route id").to_string();
        let expected_rule = runtime_ids.rule.expect("rule id").to_string();
        let expected_backend = runtime_ids.backend.expect("backend id").to_string();
        let expected_endpoint = runtime_ids.endpoint.expect("endpoint id").to_string();
        let log_path = temp_log_path("runtime-ids");
        let path_text = log_path.display().to_string();
        let traffic = SharedTrafficStats::shared();
        let mut ctx = RequestContext {
            started_at_unix_ms: 123,
            snapshot_version: "v1".to_string(),
            client_ip: "127.0.0.1".to_string(),
            host: "orders.example.com".to_string(),
            method: "GET".to_string(),
            path: "/".to_string(),
            status: 200,
            runtime_ids,
            selected_backend: Some(Arc::new(selected)),
            ..RequestContext::default()
        };

        observe_completed_request(
            &AccessLogOptions {
                enabled: true,
                path: path_text.clone(),
                mode: ntgw_observability::AccessLogMode::Json,
                ..AccessLogOptions::default()
            },
            &traffic,
            &mut ctx,
            12,
            128,
        );

        let contents = wait_for_log_contents(&log_path);
        assert!(contents.contains(&format!("\"listenerRuntimeId\":\"{expected_listener}\"")));
        assert!(contents.contains(&format!("\"routeRuntimeId\":\"{expected_route}\"")));
        assert!(contents.contains(&format!("\"ruleRuntimeId\":\"{expected_rule}\"")));
        assert!(contents.contains(&format!("\"backendRuntimeId\":\"{expected_backend}\"")));
        assert!(contents.contains(&format!("\"endpointRuntimeId\":\"{expected_endpoint}\"")));
        assert!(contents.contains("\"listener\":\"default/gw/http\""));
        assert!(contents.contains("\"routeNamespace\":\"default\""));
        assert!(contents.contains("\"routeName\":\"orders\""));
        assert!(contents.contains("\"backend\":\"default/orders:8080\""));

        shutdown_access_log_writer(&path_text);
        let _ = fs::remove_file(log_path);
    }

    #[test]
    fn observe_completed_request_records_runtime_ids_in_traffic_graph() {
        let mut snapshot = Snapshot {
            listeners: vec![Listener {
                name: "default/gw/http".to_string(),
                address: "0.0.0.0".to_string(),
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                attached_routes: vec!["default/orders".to_string()],
                ..Listener::default()
            }],
            http_routes: vec![HttpRoute {
                name: "orders".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["orders.example.com".to_string()],
                rules: vec![HttpRule {
                    name: String::new(),
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "orders".to_string(),
                        port: 8080,
                        ..BackendRef::default()
                    }],
                    ..HttpRule::default()
                }],
                ..HttpRoute::default()
            }],
            backends: vec![BackendCluster {
                ai_service: None,
                token_policy: None,
                name: "orders:8080".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
            }],
            ..Snapshot::default()
        };
        snapshot.rebuild_runtime_indexes();
        let selected = snapshot
            .select_backend(&RequestMeta::new(
                Some("orders.example.com".to_string()),
                "/",
                "GET",
                BTreeMap::new(),
            ))
            .expect("selected backend");
        let runtime_ids = snapshot.selected_backend_runtime_ids(&selected);
        let expected_listener = runtime_ids.listener.expect("listener id").to_string();
        let expected_route = runtime_ids.route.expect("route id").to_string();
        let expected_backend = runtime_ids.backend.expect("backend id").to_string();
        let traffic = SharedTrafficStats::shared();
        let mut ctx = RequestContext {
            started_at_unix_ms: 123,
            snapshot_version: "v1".to_string(),
            client_ip: "127.0.0.1".to_string(),
            host: "orders.example.com".to_string(),
            method: "GET".to_string(),
            path: "/".to_string(),
            status: 200,
            runtime_ids,
            selected_backend: Some(Arc::new(selected)),
            ..RequestContext::default()
        };

        observe_completed_request(
            &AccessLogOptions {
                enabled: false,
                ..AccessLogOptions::default()
            },
            &traffic,
            &mut ctx,
            12,
            128,
        );

        let snapshot = traffic.snapshot();
        let value = serde_json::to_value(snapshot).expect("traffic snapshot json");
        let listener_node = value["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|node| node["node_id"] == "listener:default/gw/http")
            .expect("listener node");
        let route_node = value["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|node| node["node_id"] == "route:HTTPRoute:default/orders")
            .expect("route node");
        let backend_node = value["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|node| node["node_id"] == "backend:default/orders:8080")
            .expect("backend node");

        assert_eq!(
            listener_node["runtimeId"].as_str(),
            Some(expected_listener.as_str())
        );
        assert_eq!(
            route_node["runtimeId"].as_str(),
            Some(expected_route.as_str())
        );
        assert_eq!(
            backend_node["runtimeId"].as_str(),
            Some(expected_backend.as_str())
        );
    }

    #[test]
    fn observe_completed_request_prefers_cached_local_response_topology() {
        let traffic = SharedTrafficStats::shared();
        let mut ctx = RequestContext {
            listener_name: "default/gw/http".to_string(),
            listener_protocol: "HTTP".to_string(),
            route_namespace: "stale".to_string(),
            route_name: "stale".to_string(),
            route_kind: "Http".to_string(),
            status: 404,
            response_flags: "NR".to_string(),
            local_response_traffic_topology: Some(Arc::new(
                ntgw_observability::TrafficTopology::unmatched("default/gw/http"),
            )),
            ..RequestContext::default()
        };

        observe_completed_request(
            &AccessLogOptions {
                enabled: false,
                ..AccessLogOptions::default()
            },
            &traffic,
            &mut ctx,
            7,
            256,
        );

        let snapshot = traffic.snapshot();
        assert!(
            snapshot
                .nodes
                .iter()
                .any(|node| node.node_id == "route:UnmatchedRoute:unmatched/no-route")
        );
        assert!(
            !snapshot
                .nodes
                .iter()
                .any(|node| node.node_id == "route:HTTPRoute:stale/stale")
        );
    }

    #[test]
    fn access_log_sample_key_uses_runtime_ids_as_numeric_keys() {
        let mut snapshot = Snapshot {
            listeners: vec![Listener {
                name: "default/gw/http".to_string(),
                address: "0.0.0.0".to_string(),
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                attached_routes: vec!["default/orders".to_string()],
                ..Listener::default()
            }],
            http_routes: vec![HttpRoute {
                name: "orders".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["orders.example.com".to_string()],
                rules: vec![HttpRule {
                    name: String::new(),
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string(),
                        name: "orders".to_string(),
                        port: 8080,
                        ..BackendRef::default()
                    }],
                    ..HttpRule::default()
                }],
                ..HttpRoute::default()
            }],
            backends: vec![BackendCluster {
                ai_service: None,
                token_policy: None,
                name: "orders:8080".to_string().into(),
                namespace: "default".to_string().into(),
                protocol: "HTTP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
            }],
            ..Snapshot::default()
        };
        snapshot.rebuild_runtime_indexes();
        let selected = snapshot
            .select_backend(&RequestMeta::new(
                Some("orders.example.com".to_string()),
                "/",
                "GET",
                BTreeMap::new(),
            ))
            .expect("selected backend");
        let runtime_ids = snapshot.selected_backend_runtime_ids(&selected);
        let ctx = RequestContext {
            started_at_unix_ms: 123,
            request_id: "request-1".to_string(),
            runtime_ids,
            selected_backend: Some(Arc::new(selected)),
            ..RequestContext::default()
        };

        let key = access_log_sample_key(&ctx);

        assert_eq!(
            key.listener_runtime_id,
            runtime_ids.listener.map(|id| id.as_u64())
        );
        assert_eq!(
            key.route_runtime_id,
            runtime_ids.route.map(|id| id.as_u64())
        );
        assert_eq!(
            key.backend_runtime_id,
            runtime_ids.backend.map(|id| id.as_u64())
        );
    }

    fn temp_log_path(prefix: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("ntgw-http-proxy-{prefix}-{unique}.log"))
    }

    fn wait_for_log_contents(path: &Path) -> String {
        for _ in 0..20 {
            if let Ok(contents) = fs::read_to_string(path)
                && !contents.trim().is_empty() {
                    return contents;
                }
            thread::sleep(Duration::from_millis(20));
        }

        fs::read_to_string(path).expect("access log file")
    }
}
