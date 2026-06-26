use std::collections::BTreeMap;

use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendRef, BackendSelectionError, HttpRoute, HttpRule,
    Listener, RequestMeta, Snapshot,
};

#[path = "invalid_backend_refs/invalid_ref.rs"]
mod invalid_ref;
#[path = "invalid_backend_refs/mixed_refs.rs"]
mod mixed_refs;
#[path = "invalid_backend_refs/serviceimport.rs"]
mod serviceimport;
#[path = "invalid_backend_refs/unhealthy.rs"]
mod unhealthy;

fn listener(name: &str, attached_route: &str) -> Listener {
    Listener {
        name: name.to_string(),
        address: "0.0.0.0".to_string(),
        addresses: vec!["0.0.0.0".to_string()],
        port: 80,
        protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        hostnames: vec![],
        attached_routes: vec![attached_route.to_string()],
        tls: None,
        backend_tls: None,
        metadata: BTreeMap::new(),
    }
}

fn backend_cluster(namespace: &str, name: &str, healthy: bool) -> BackendCluster {
    BackendCluster {
        name: format!("{name}:8080"),
        namespace: namespace.to_string(),
        protocol: "HTTP".to_string(),
        endpoints: vec![BackendEndpoint {
            address: if healthy {
                "10.0.0.2".to_string()
            } else {
                "10.0.0.3".to_string()
            },
            port: 8080,
            healthy,
        }],
        wasm_plugin: None,
        ai_service: None,
        token_policy: None,
        circuit_breaker: None,
    }
}
