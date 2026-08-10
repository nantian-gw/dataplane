use super::super::*;
use super::backend_ref_is_routable;
use std::time::Instant;

#[test]
fn collect_http_backend_candidates_preserves_backend_names() {
    let snapshot = Snapshot {
        backends: vec![BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
            ai_service: None,
            token_policy: None,
            circuit_breaker: None,
        }],
        ..Snapshot::default()
    };
    let refs = vec![BackendRef {
        namespace: "default".to_string(),
        name: "orders".to_string(),
        port: 8080,
        weight: 1,
        ..BackendRef::default()
    }];

    let (candidates, saw_invalid_refs, saw_unhealthy_backend) =
        snapshot.collect_http_backend_candidates(&refs, Instant::now());

    assert!(!saw_invalid_refs);
    assert!(!saw_unhealthy_backend);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].backend_name, "default/orders:8080");
}

#[test]
fn visit_http_backend_candidates_preserves_order_and_status() {
    let snapshot = Snapshot {
        backends: vec![
            BackendCluster {
                name: "users:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.10".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
                circuit_breaker: None,
            },
            BackendCluster {
                name: "orders:8081".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.11".to_string(),
                    port: 8081,
                    healthy: false,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
                circuit_breaker: None,
            },
            BackendCluster {
                name: "payments:8082".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.12".to_string(),
                    port: 8082,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
                circuit_breaker: None,
            },
        ],
        ..Snapshot::default()
    };
    let refs = vec![
        BackendRef {
            namespace: "default".to_string(),
            name: "users".to_string(),
            port: 8080,
            weight: 2,
            ..BackendRef::default()
        },
        BackendRef {
            namespace: "default".to_string(),
            name: "ignored-zero".to_string(),
            port: 8088,
            weight: 0,
            ..BackendRef::default()
        },
        BackendRef {
            namespace: "default".to_string(),
            name: "invalid".to_string(),
            port: 8089,
            weight: 1,
            metadata: BTreeMap::from([(
                BACKEND_REF_META_VALID.to_string(),
                "false".to_string(),
            )]),
            ..BackendRef::default()
        },
        BackendRef {
            namespace: "default".to_string(),
            name: "orders".to_string(),
            port: 8081,
            weight: 4,
            ..BackendRef::default()
        },
        BackendRef {
            namespace: "default".to_string(),
            name: "payments".to_string(),
            port: 8082,
            weight: 3,
            ..BackendRef::default()
        },
    ];

    let mut visited = Vec::new();
    let outcome = snapshot.visit_http_backend_candidates(&refs, Instant::now(), |candidate| {
        visited.push((
            candidate.backend_name.into_owned(),
            candidate.backend_ref.weight,
        ));
        true
    });

    assert_eq!(
        visited,
        vec![
            ("default/users:8080".to_string(), 2),
            ("default/payments:8082".to_string(), 3),
        ]
    );
    assert_eq!(outcome.candidate_count, 2);
    assert_eq!(outcome.total_weight, 5);
    assert!(outcome.saw_invalid_refs);
    assert!(outcome.saw_unhealthy_backend);
}

#[test]
fn backend_policy_helpers_borrow_snapshot_values() {
    let snapshot = Snapshot {
        backend_policies: BTreeMap::from([(
            "default/users:8080".to_string(),
            BackendPolicy {
                session_persistence: Some(SessionPersistence {
                    session_name: "ntgw-session".to_string(),
                    session_type: "Header".to_string(),
                    ..SessionPersistence::default()
                }),
                load_balancing: Some(LoadBalancingPolicy {
                    policy_type: "ConsistentHash".to_string(),
                    consistent_hash: Some(ConsistentHashPolicy {
                        key_type: "Header".to_string(),
                        header_name: "x-user-id".to_string(),
                    }),
                }),
                ..BackendPolicy::default()
            },
        )]),
        ..Snapshot::default()
    };
    let stored = snapshot.backend_policy("default/users:8080").unwrap();

    let session = snapshot
        .backend_session_persistence("default/users:8080")
        .unwrap();
    let load_balancing = snapshot
        .backend_load_balancing("default/users:8080")
        .unwrap();

    assert!(std::ptr::eq(
        session,
        stored.session_persistence.as_ref().unwrap()
    ));
    assert!(std::ptr::eq(
        load_balancing,
        stored.load_balancing.as_ref().unwrap()
    ));
}

#[test]
fn visit_http_backend_candidates_borrows_indexed_backend_names() {
    let mut snapshot = Snapshot {
        backends: vec![BackendCluster {
            name: "users:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
            ai_service: None,
            token_policy: None,
            circuit_breaker: None,
        }],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();
    let refs = vec![BackendRef {
        namespace: "default".to_string(),
        name: "users".to_string(),
        port: 8080,
        weight: 1,
        ..BackendRef::default()
    }];

    let mut saw_borrowed_backend_name = false;
    snapshot.visit_http_backend_candidates(&refs, Instant::now(), |candidate| {
        match candidate.backend_name {
            std::borrow::Cow::Borrowed(name) => {
                saw_borrowed_backend_name = true;
                assert!(std::ptr::eq(name, snapshot.backend_names[0].as_ref()));
            }
            std::borrow::Cow::Owned(_) => panic!("expected indexed backend name borrow"),
        }
        true
    });

    assert!(saw_borrowed_backend_name);
}

#[test]
fn endpoint_rendezvous_hash_matches_string_port_parts() {
    for port in [0, 1, 80, 8080, u32::MAX] {
        let endpoint = BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port,
            healthy: true,
        };
        let port_text = port.to_string();

        assert_eq!(
            rendezvous_hash_endpoint("hash-key", "default/orders:8080", &endpoint),
            rendezvous_hash(&[
                "hash-key",
                "default/orders:8080",
                endpoint.address.as_str(),
                port_text.as_str(),
            ])
        );
    }
}