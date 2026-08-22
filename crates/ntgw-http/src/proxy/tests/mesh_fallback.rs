use std::collections::BTreeMap;

use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendRef, HttpMatch, HttpRoute, HttpRule, Listener,
    ParentRef, QueryMatch, RequestMeta, Snapshot, Workload,
};

use super::super::select_backend_after_http_route_miss;
use super::super::selection::SelectedBackendConfigCache;

#[test]
fn http_route_miss_uses_mesh_default_backend_for_ineligible_cross_namespace_route() {
    // After removing the source_namespace check, cross-namespace mesh routes
    // are accepted regardless of the source workload's namespace. The route
    // should be selected directly, not via the fallback path.
    let mut snapshot = Snapshot {
        listeners: vec![mesh_listener(
            "gateway-conformance-mesh",
            "echo-v1",
            80,
            20080,
            "HTTP",
            &["gateway-conformance-mesh-consumer/mesh-echo-add-header"],
        )],
        http_routes: vec![HttpRoute {
            name: "mesh-echo-add-header".to_string(),
            namespace: "gateway-conformance-mesh-consumer".to_string(),
            hostnames: vec![],
            parent_refs: vec![ParentRef {
                kind: "Service".to_string(),
                namespace: "gateway-conformance-mesh".to_string(),
                name: "echo-v1".to_string(),
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![BackendRef {
                    namespace: "gateway-conformance-mesh".to_string(),
                    name: "echo-v1".to_string(),
                    port: 80,
                    ..BackendRef::default()
                }],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "echo-v1:80".to_string(),
            namespace: "gateway-conformance-mesh".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.11".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
            circuit_breaker: None,
            security_policy: None,
        }],
        workloads: vec![
            Workload {
                namespace: "gateway-conformance-mesh-consumer".to_string(),
                name: "consumer".to_string(),
                ip: "10.1.0.10".to_string(),
            },
            Workload {
                namespace: "gateway-conformance-mesh".to_string(),
                name: "producer".to_string(),
                ip: "10.1.0.20".to_string(),
            },
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    let mut request = RequestMeta::with_port(
        Some("echo-v1.gateway-conformance-mesh".to_string()),
        20080,
        "/",
        "GET",
        BTreeMap::new(),
    );
    request.source_ip = Some("10.1.0.20".to_string());

    // The route should be selected directly (not via fallback path)
    let selected = snapshot.select_http_route(&request);
    assert!(
        selected.is_some(),
        "cross-namespace mesh route should be accepted"
    );
    let selected = selected.unwrap();
    assert_eq!(selected.route_name, "mesh-echo-add-header");
    assert_eq!(
        selected.route_namespace,
        "gateway-conformance-mesh-consumer"
    );
}

#[test]
fn http_route_miss_preserves_no_route_for_attached_mesh_rule_miss() {
    let mut snapshot = Snapshot {
        listeners: vec![mesh_listener(
            "default",
            "echo",
            80,
            20080,
            "HTTP",
            &["default/query-param"],
        )],
        http_routes: vec![HttpRoute {
            name: "query-param".to_string(),
            namespace: "default".to_string(),
            hostnames: vec![],
            parent_refs: vec![ParentRef {
                kind: "Service".to_string(),
                name: "echo".to_string(),
                port: 80,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    query_params: vec![QueryMatch {
                        name: "mode".to_string(),
                        value: "canary".to_string(),
                        ..QueryMatch::default()
                    }],
                    ..HttpMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "echo".to_string(),
                    port: 80,
                    ..BackendRef::default()
                }],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "echo:80".to_string(),
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

    let cache = SelectedBackendConfigCache;
    let selected = select_backend_after_http_route_miss(
        &cache,
        &snapshot,
        &RequestMeta::with_port(
            Some("echo.default".to_string()),
            20080,
            "/?mode=stable",
            "GET",
            BTreeMap::new(),
        ),
        &|_| None,
    )
    .expect("selection should not error");

    assert!(selected.is_none());
}

fn mesh_listener(
    namespace: &str,
    name: &str,
    service_port: u32,
    listen_port: u32,
    protocol: &str,
    attached_routes: &[&str],
) -> Listener {
    let mut listener = Listener {
        name: format!("mesh/{namespace}/{name}/{listen_port}"),
        address: "0.0.0.0".to_string(),
        port: listen_port,
        protocol: protocol.to_string(),
        hostnames: vec![],
        attached_routes: attached_routes
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
        tls: None,
        ..Listener::default()
    };
    listener.metadata.insert(
        "nantian.dev/frontend-kind".to_string(),
        "Service".to_string(),
    );
    listener.metadata.insert(
        "nantian.dev/frontend-namespace".to_string(),
        namespace.to_string(),
    );
    listener
        .metadata
        .insert("nantian.dev/frontend-name".to_string(), name.to_string());
    listener.metadata.insert(
        "nantian.dev/frontend-port".to_string(),
        service_port.to_string(),
    );
    listener
}
