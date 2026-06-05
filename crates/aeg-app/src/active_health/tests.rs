use std::{collections::BTreeMap, time::Duration};

use tokio::net::TcpListener;

use aeg_ir::{
    BackendCluster, BackendEndpoint, BackendRef, HttpRoute, HttpRule, RequestMeta, Snapshot,
};

use super::{
    apply_probe_results, collect_probe_targets, probe_target_once, ProbeResult, ProbeTarget,
};

#[test]
fn collect_probe_targets_skips_udp_and_statically_unhealthy_endpoints() {
    let snapshot = Snapshot {
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: "http:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![
                    BackendEndpoint {
                        address: "10.0.0.10".to_string(),
                        port: 8080,
                        healthy: true,
                    },
                    BackendEndpoint {
                        address: "10.0.0.11".to_string(),
                        port: 8080,
                        healthy: false,
                    },
                ],
                wasm_plugin: None,
            },
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: "udp:5353".to_string(),
                namespace: "default".to_string(),
                protocol: "UDP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.20".to_string(),
                    port: 5353,
                    healthy: true,
                }],
                wasm_plugin: None,
            },
        ],
        ..Snapshot::default()
    };

    let targets = collect_probe_targets(&snapshot);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].backend_name, "default/http:8080");
    assert_eq!(targets[0].address, "10.0.0.10");
    assert_eq!(targets[0].port, 8080);
}

#[tokio::test]
async fn probe_target_once_reflects_tcp_connectivity() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test listener");
    let addr = listener.local_addr().expect("listener address");

    let healthy_target = ProbeTarget {
        backend_name: "default/http:8080".to_string(),
        address: addr.ip().to_string(),
        port: addr.port() as u32,
    };
    assert!(probe_target_once(&healthy_target, Duration::from_millis(200)).await);

    drop(listener);

    assert!(!probe_target_once(&healthy_target, Duration::from_millis(200)).await);
}

#[test]
fn apply_probe_results_updates_snapshot_runtime_health() {
    let mut snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "echo".to_string(),
                    port: 8080,
                    ..BackendRef::default()
                }],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "echo:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![
                BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                },
                BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8080,
                    healthy: true,
                },
            ],
            wasm_plugin: None,
        }],
        ..Snapshot::default()
    };
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    apply_probe_results(
        &mut snapshot,
        &[ProbeResult {
            target: ProbeTarget {
                backend_name: "default/echo:8080".to_string(),
                address: "10.0.0.10".to_string(),
                port: 8080,
            },
            healthy: false,
        }],
        1,
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    assert_eq!(selected.backend.address, "10.0.0.11");

    apply_probe_results(
        &mut snapshot,
        &[ProbeResult {
            target: ProbeTarget {
                backend_name: "default/echo:8080".to_string(),
                address: "10.0.0.10".to_string(),
                port: 8080,
            },
            healthy: true,
        }],
        1,
    );

    let recovered_addresses: Vec<_> = (0..4)
        .map(|_| {
            snapshot
                .select_backend(&request)
                .expect("backend")
                .backend
                .address
        })
        .collect();
    assert!(recovered_addresses
        .iter()
        .any(|address| address == "10.0.0.10"));
    assert!(recovered_addresses
        .iter()
        .any(|address| address == "10.0.0.11"));
}
