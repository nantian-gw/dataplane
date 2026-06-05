#[test]
fn runtime_indexes_precompute_request_header_materialization_requirements() {
    let mut snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "header-route".to_string(),
            namespace: "default".to_string(),
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    headers: vec![HeaderMatch {
                        name: "x-env".to_string(),
                        value: "prod".to_string(),
                        match_type: "Exact".to_string(),
                        ..HeaderMatch::default()
                    }],
                    ..HttpMatch::default()
                }],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        grpc_routes: vec![GrpcRoute {
            name: "grpc".to_string(),
            namespace: "default".to_string(),
            rules: vec![GrpcRule {
                name: String::new(),
                session_persistence: Some(SessionPersistence {
                    session_name: "aeg-session".to_string(),
                    session_type: "Cookie".to_string(),
                    ..SessionPersistence::default()
                }),
                ..GrpcRule::default()
            }],
            ..GrpcRoute::default()
        }],
        backend_policies: BTreeMap::from([(
            "default/backend:8080".to_string(),
            crate::BackendPolicy {
                load_balancing: Some(crate::LoadBalancingPolicy {
                    policy_type: "ConsistentHash".to_string(),
                    consistent_hash: Some(crate::ConsistentHashPolicy {
                        key_type: "Header".to_string(),
                        header_name: "x-user-id".to_string(),
                    }),
                }),
                ..BackendPolicy::default()
            },
        )]),
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert!(snapshot.request_materialization.http_route_headers);
    assert!(snapshot.request_materialization.session_headers);
    assert!(snapshot.request_materialization.backend_hash_headers);
    assert!(!snapshot.request_materialization.source_ip);
    assert!(snapshot.request_materialization.requires_full_headers());
}

#[test]
fn runtime_indexes_precompute_source_ip_materialization_for_source_hash() {
    let mut snapshot = Snapshot {
        backend_policies: BTreeMap::from([(
            "default/backend:8080".to_string(),
            crate::BackendPolicy {
                load_balancing: Some(crate::LoadBalancingPolicy {
                    policy_type: "ConsistentHash".to_string(),
                    consistent_hash: Some(crate::ConsistentHashPolicy {
                        key_type: "SourceIP".to_string(),
                        header_name: String::new(),
                    }),
                }),
                ..BackendPolicy::default()
            },
        )]),
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert!(snapshot.request_materialization.source_ip);
    assert!(!snapshot.request_materialization.requires_full_headers());
}

#[test]
fn runtime_indexes_precompute_source_ip_materialization_for_workloads() {
    let mut snapshot = Snapshot {
        workloads: vec![Workload {
            namespace: "default".to_string(),
            name: "client".to_string(),
            ip: "10.1.2.3".to_string(),
        }],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert!(snapshot.request_materialization.source_ip);
    assert!(!snapshot.request_materialization.requires_full_headers());
}
