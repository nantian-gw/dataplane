use std::collections::BTreeMap;

use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendRef, BackendTlsConfig, HttpRoute, HttpRule, Listener,
    RequestMeta, Snapshot,
};
use ntgw_proto::gateway::control::v1 as proto;

#[test]
fn decodes_listener_backend_tls_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        listeners: vec![proto::Listener {
            name: "default/gw/https".to_string(),
            address: "192.0.2.10".to_string(),
            addresses: vec!["192.0.2.10".to_string(), "gw.example.com".to_string()],
            backend_tls: Some(proto::BackendTlsConfig {
                client_certificate_ref: "default/client-cert".to_string(),
            }),
            ..Default::default()
        }],
        ..Default::default()
    });

    let backend_tls = snapshot.listeners[0]
        .backend_tls
        .as_ref()
        .expect("backend tls");
    assert_eq!(snapshot.listeners[0].address, "192.0.2.10");
    assert_eq!(
        snapshot.listeners[0].addresses,
        vec!["192.0.2.10".to_string(), "gw.example.com".to_string()]
    );
    assert_eq!(backend_tls.client_certificate_ref, "default/client-cert");
}

#[test]
fn select_http_backend_preserves_listener_backend_tls() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/https".to_string(),
            port: 443,
            protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
            attached_routes: vec!["default/route".to_string()],
            backend_tls: Some(BackendTlsConfig {
                client_certificate_ref: "default/client-cert".to_string(),
            }),
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            rules: vec![HttpRule {
                name: String::new(),
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "echo".to_string(),
                    port: 8443,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            name: "echo:8443".into(),
            namespace: "default".into(),
            protocol: "HTTPS".into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8443,
                healthy: true,
            }],
            wasm_plugin: None,
            ai_service: None,
            token_policy: None,

            circuit_breaker: None,
            security_policy: None,
        }],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_backend(&RequestMeta::with_port(
            Some("example.com".to_string()),
            443,
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("selected backend");

    let backend_tls = selected.backend_tls.as_ref().expect("backend tls");
    assert_eq!(backend_tls.client_certificate_ref, "default/client-cert");
}
