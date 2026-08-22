mod completion;
mod helpers;
mod route_labels;
mod types;

pub(crate) use completion::observe_completed_request;

// Re-exports for cross-submodule access and test module visibility.
// These appear unused in lib builds but are required by `use super::*` in #[cfg(test)].
#[allow(unused_imports)]
pub(crate) use super::request::access_log_route_annotations;
#[allow(unused_imports)]
pub(crate) use completion::access_log_sample_key;
#[allow(unused_imports)]
pub(crate) use helpers::{build_request_line, extract_request_header};
#[allow(unused_imports)]
pub(crate) use route_labels::request_route_labels;
#[allow(unused_imports)]
pub(crate) use types::RequestRouteLabels;

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
    use ntgw_observability::{AccessLogOptions, SharedTrafficStats, shutdown_access_log_writer};

    use super::super::context::RequestContext;
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
                Arc::from("user-agent"),
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
                Arc::from("content-type"),
                "application/json".to_string(),
            )]),
            access_log_upstream_response_headers: BTreeMap::from([(
                Arc::from("server"),
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
                name: "orders:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
                security_policy: None,
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
                name: "orders:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
                security_policy: None,
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
                name: "orders:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,

                circuit_breaker: None,
                security_policy: None,
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
                && !contents.trim().is_empty()
            {
                return contents;
            }
            thread::sleep(Duration::from_millis(20));
        }

        fs::read_to_string(path).expect("access log file")
    }
}
