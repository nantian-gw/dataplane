#[test]
fn runtime_indexes_accelerate_backend_secret_and_workload_lookups() {
    let mut snapshot = Snapshot {
        backends: vec![BackendCluster {
            name: "api:8443".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTPS".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 8443,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        }],
        secrets: vec![SecretMaterial {
            namespace: "default".to_string(),
            name: "client-cert".to_string(),
            cert_pem: "cert".to_string(),
            key_pem: "key".to_string(),
        }],
        workloads: vec![Workload {
            name: "dp-1".to_string(),
            namespace: "tenant-a".to_string(),
            ip: "10.1.2.3".to_string(),
        }],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert_eq!(snapshot.backend_protocol("default/api:8443"), Some("HTTPS"));
    let secret = snapshot
        .secret_material("default", "client-cert")
        .expect("secret");
    assert_eq!(secret.cert_pem, "cert");
    assert_eq!(
        snapshot.source_namespace(&RequestMeta {
            source_ip: Some("10.1.2.3".to_string()),
            ..RequestMeta::default()
        }),
        Some("tenant-a")
    );
}

#[test]
fn unbuilt_backend_index_does_not_override_slow_path_backend_lookup() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "orders-route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["orders.example.com".to_string()],
            rules: vec![HttpRule {
                backend_refs: vec![backend_ref("default", "orders", 8080)],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![
            BackendCluster {
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
            },
            BackendCluster {
                name: "payments:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTPS".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.20".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
        ],
        backend_index: std::collections::HashMap::from([(
            std::sync::Arc::<str>::from("default/orders:8080"),
            1,
        )]),
        runtime_indexes_ready: false,
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_backend(&RequestMeta::new(
            Some("orders.example.com".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("backend");

    assert_eq!(selected.backend_name, "default/orders:8080");
    assert_eq!(selected.backend.address, "10.0.0.10");
    assert_eq!(snapshot.backend_protocol("default/orders:8080"), Some("HTTP"));
}

#[test]
fn runtime_indexes_precompute_backend_lookup_for_backend_refs() {
    let mut snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "orders-route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["orders.example.com".to_string()],
            rules: vec![HttpRule {
                backend_refs: vec![backend_ref("default", "orders", 8080)],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![
            BackendCluster {
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
            },
            BackendCluster {
                name: "payments:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTPS".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "10.0.0.20".to_string(),
                    port: 8080,
                    healthy: true,
                }],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
        ],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();
    snapshot
        .backend_index
        .insert(std::sync::Arc::<str>::from("default/orders:8080"), 1);

    let selected = snapshot
        .select_backend(&RequestMeta::new(
            Some("orders.example.com".to_string()),
            "/",
            "GET",
            BTreeMap::new(),
        ))
        .expect("backend");

    assert_eq!(selected.backend_name, "default/orders:8080");
    assert_eq!(selected.backend.address, "10.0.0.10");
}

#[test]
fn backend_lookup_requires_an_exact_port_string_match() {
    let snapshot = Snapshot {
        backends: vec![BackendCluster {
            name: "orders:08080".to_string(),
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
        }],
        runtime_indexes_ready: false,
        ..Snapshot::default()
    };

    assert_eq!(snapshot.backend_protocol("default/orders:8080"), None);
}

#[test]
fn backend_ref_lookup_requires_an_exact_port_string_match() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "orders-route".to_string(),
            namespace: "default".to_string(),
            hostnames: vec!["orders.example.com".to_string()],
            rules: vec![HttpRule {
                backend_refs: vec![backend_ref("default", "orders", 8080)],
                ..HttpRule::default()
            }],
            ..HttpRoute::default()
        }],
        backends: vec![BackendCluster {
            name: "orders:08080".to_string(),
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
        }],
        runtime_indexes_ready: false,
        ..Snapshot::default()
    };

    assert!(
        snapshot
            .select_backend(&RequestMeta::new(
                Some("orders.example.com".to_string()),
                "/",
                "GET",
                BTreeMap::new(),
            ))
            .is_none()
    );
}

#[test]
fn unbuilt_secret_index_does_not_override_slow_path_secret_lookup() {
    let snapshot = Snapshot {
        secrets: vec![
            SecretMaterial {
                namespace: "default".to_string(),
                name: "client-cert".to_string(),
                cert_pem: "expected-cert".to_string(),
                key_pem: "expected-key".to_string(),
            },
            SecretMaterial {
                namespace: "default".to_string(),
                name: "other-cert".to_string(),
                cert_pem: "stale-cert".to_string(),
                key_pem: "stale-key".to_string(),
            },
        ],
        secret_index: std::collections::HashMap::from([(
            "default/client-cert".to_string(),
            1,
        )]),
        runtime_indexes_ready: false,
        ..Snapshot::default()
    };

    let secret = snapshot
        .secret_material("default", "client-cert")
        .expect("secret");

    assert_eq!(secret.cert_pem, "expected-cert");
    assert_eq!(secret.key_pem, "expected-key");
}

#[test]
fn unbuilt_workload_index_does_not_override_slow_path_source_namespace_lookup() {
    let snapshot = Snapshot {
        workloads: vec![Workload {
            namespace: "tenant-a".to_string(),
            name: "client".to_string(),
            ip: "10.1.2.3".to_string(),
        }],
        workload_namespace_index: std::collections::HashMap::from([(
            "10.1.2.3".to_string(),
            "tenant-b".to_string(),
        )]),
        runtime_indexes_ready: false,
        ..Snapshot::default()
    };

    assert_eq!(
        snapshot.source_namespace(&RequestMeta {
            source_ip: Some("10.1.2.3".to_string()),
            ..RequestMeta::default()
        }),
        Some("tenant-a")
    );
}

#[test]
fn runtime_indexes_precompute_backend_service_namespace_lookup() {
    let mut snapshot = Snapshot {
        backends: vec![
            BackendCluster {
                name: "orders:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
            BackendCluster {
                name: "payments:9090".to_string(),
                namespace: "tenant-a".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
            BackendCluster {
                name: "shared:8080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
            BackendCluster {
                name: "shared:8080".to_string(),
                namespace: "tenant-b".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
            BackendCluster {
                name: "padded:08080".to_string(),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![],
                wasm_plugin: None,
                ai_service: None,
                token_policy: None,
            },
        ],
        ..Snapshot::default()
    };

    snapshot.rebuild_runtime_indexes();

    assert_eq!(
        snapshot
            .backend_namespace_for_service("orders", 8080)
            .as_deref(),
        Some("default")
    );
    assert_eq!(
        snapshot
            .backend_namespace_for_service("payments", 9090)
            .as_deref(),
        Some("tenant-a")
    );
    assert!(
        snapshot
            .backend_namespace_for_service("shared", 8080)
            .is_none()
    );
    assert!(
        snapshot
            .backend_namespace_for_service("padded", 8080)
            .is_none()
    );
}

#[test]
fn backend_service_index_tracks_name_buckets_with_ports_and_namespaces() {
    let mut index = crate::BackendServiceIndex::with_capacity(3);

    index.insert("default", "orders", 8080, 0);
    index.insert("default", "orders", 9090, 1);
    index.insert("tenant-a", "orders", 8080, 2);
    index.insert("default", "orders", 8080, 3);

    assert_eq!(index.index_for("default", "orders", 8080), Some(3));
    assert_eq!(index.index_for("default", "orders", 9090), Some(1));
    assert_eq!(index.index_for("tenant-a", "orders", 8080), Some(2));
    assert_eq!(index.unique_namespace("orders", 9090), Some("default"));
    assert!(index.unique_namespace("orders", 8080).is_none());
    assert_eq!(index.service_name_count(), 1);
    assert_eq!(index.entry_count(), 3);
}

#[test]
fn unbuilt_backend_service_index_does_not_override_slow_path_namespace_lookup() {
    let mut stale_index = crate::BackendServiceIndex::with_capacity(1);
    stale_index.insert("stale", "orders", 8080, 0);

    let snapshot = Snapshot {
        backends: vec![BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        }],
        backend_service_index: stale_index,
        runtime_indexes_ready: false,
        ..Snapshot::default()
    };

    assert_eq!(
        snapshot
            .backend_namespace_for_service("orders", 8080)
            .as_deref(),
        Some("default")
    );
}
