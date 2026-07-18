use std::collections::BTreeMap;

use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendRef, GrpcMatch, GrpcRoute, GrpcRule, HeaderMatch,
    HttpMatch, HttpRoute, HttpRule, Listener, RequestMeta, RouteKind, Snapshot,
};

#[path = "http_route_selection/hostnames_and_listeners.rs"]
mod hostnames_and_listeners;
#[path = "http_route_selection/path_and_route_kind.rs"]
mod path_and_route_kind;
#[path = "http_route_selection/rule_priority.rs"]
mod rule_priority;

fn path_rule(path: &str, namespace: &str, backend: &str, port: u32) -> HttpRule {
    HttpRule {
        name: String::new(),
        matches: vec![HttpMatch {
            path: path.to_string(),
            path_type: "PathPrefix".to_string(),
            ..HttpMatch::default()
        }],
        filters: vec![],
        backend_refs: vec![backend_ref(namespace, backend, port)],
        timeouts: None,
        retry: None,
        session_persistence: None,
    }
}

fn backend_ref(namespace: &str, name: &str, port: u32) -> BackendRef {
    BackendRef {
        namespace: namespace.to_string(),
        name: name.to_string(),
        port,
        weight: 1,
        ..BackendRef::default()
    }
}

fn backend_cluster(namespace: &str, name: &str, address: &str) -> BackendCluster {
    BackendCluster {
        name: format!("{name}:8080").into(),
        namespace: namespace.to_string().into(),
        protocol: "HTTP".to_string().into(),
        endpoints: vec![BackendEndpoint {
            address: address.to_string(),
            port: 8080,
            healthy: true,
        }],
        wasm_plugin: None,
        ai_service: None,
        token_policy: None,
        circuit_breaker: None,
    }
}

fn listener_with_hostnames(name: &str, hostnames: &[&str], attached_routes: &[&str]) -> Listener {
    Listener {
        name: name.to_string(),
        address: "0.0.0.0".to_string(),
        addresses: vec!["0.0.0.0".to_string()],
        port: 80,
        protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        hostnames: hostnames.iter().map(|item| (*item).to_string()).collect(),
        attached_routes: attached_routes
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
        tls: None,
        backend_tls: None,
        metadata: BTreeMap::new(),
    }
}

fn headers(values: &[(&str, &str)]) -> BTreeMap<String, Vec<String>> {
    values
        .iter()
        .fold(BTreeMap::new(), |mut acc, (name, value)| {
            acc.entry((*name).to_string())
                .or_default()
                .push((*value).to_string());
            acc
        })
}
