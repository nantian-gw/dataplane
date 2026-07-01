fn runtime_store_test_snapshot() -> Snapshot {
    Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: vec!["api.example.com".to_string()],
            parent_refs: vec![],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![],
                filters: vec![],
                backend_refs: vec![backend_ref("default", "echo", 8080)],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "echo:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
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
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    }
}

#[test]
fn endpoint_runtime_handle_updates_without_snapshot_mut() {
    let snapshot = runtime_store_test_snapshot();
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );
    let selected = snapshot.select_backend(&request).expect("backend");
    let handle = snapshot.endpoint_runtime_handle(&selected);

    for _ in 0..PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
        handle.record_failure(Instant::now());
    }

    assert_eq!(
        collect_http_addresses(&snapshot, &request, 4),
        vec![
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
        ]
    );
}

#[test]
fn endpoint_runtime_record_methods_update_without_snapshot_mut() {
    let snapshot = runtime_store_test_snapshot();
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );
    let selected = snapshot.select_backend(&request).expect("backend");

    for _ in 0..PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
        snapshot.record_endpoint_failure_at(&selected, Instant::now());
    }

    assert_eq!(
        collect_http_addresses(&snapshot, &request, 2),
        vec!["10.0.0.11".to_string(), "10.0.0.11".to_string()]
    );

    snapshot.record_endpoint_success(&selected);
    snapshot.record_endpoint_active_probe_failure(
        selected.backend_name.as_str(),
        &selected.backend,
        1,
    );
    assert_eq!(
        collect_http_addresses(&snapshot, &request, 2),
        vec!["10.0.0.11".to_string(), "10.0.0.11".to_string()]
    );

    snapshot
        .record_endpoint_active_probe_success(selected.backend_name.as_str(), &selected.backend);
    let addresses = collect_http_addresses(&snapshot, &request, 4);
    assert_eq!(
        addresses
            .iter()
            .filter(|address| address.as_str() == "10.0.0.10")
            .count(),
        2
    );
}

#[test]
fn cloned_snapshot_keeps_point_in_time_endpoint_runtime_view() {
    let snapshot = runtime_store_test_snapshot();
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );
    let selected = snapshot.select_backend(&request).expect("backend");
    let live_handle = snapshot.endpoint_runtime_handle(&selected);
    let frozen = snapshot.clone();

    for _ in 0..PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
        live_handle.record_failure(Instant::now());
    }

    let frozen_addresses = collect_http_addresses(&frozen, &request, 4);
    assert_eq!(
        frozen_addresses
            .iter()
            .filter(|address| address.as_str() == "10.0.0.10")
            .count(),
        2
    );
    assert_eq!(
        frozen_addresses
            .iter()
            .filter(|address| address.as_str() == "10.0.0.11")
            .count(),
        2
    );
    assert_eq!(
        collect_http_addresses(&snapshot, &request, 4),
        vec![
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
        ]
    );
}

#[test]
fn inherited_runtime_state_prunes_removed_endpoint_keys() {
    let current = runtime_store_test_snapshot();
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );
    let selected = current.select_backend(&request).expect("backend");
    let handle = current.endpoint_runtime_handle(&selected);
    for _ in 0..PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
        handle.record_failure(Instant::now());
    }

    let mut next = Snapshot {
        http_routes: current.http_routes.clone(),
        backends: vec![BackendCluster {
            name: "echo:8080".to_string().into(),
            namespace: "default".to_string().into(),
            protocol: "HTTP".to_string().into(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.11".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        
                circuit_breaker: None,}],
        ..Snapshot::default()
    };
    next.inherit_runtime_state_from(&current);

    assert!(next.endpoint_runtime.snapshot_map().is_empty());
}

#[test]
fn inherited_runtime_state_preserves_endpoint_recovery_latency_stats() {
    let current = runtime_store_test_snapshot();
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );
    let selected = current.select_backend(&request).expect("backend");
    let ejected_at = Instant::now() - Duration::from_secs(61);
    for _ in 0..PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
        current.record_endpoint_failure_at(&selected, ejected_at);
    }
    current.record_endpoint_success(&selected);

    let mut next = Snapshot {
        http_routes: current.http_routes.clone(),
        backends: current.backends.clone(),
        ..Snapshot::default()
    };
    next.inherit_runtime_state_from(&current);

    let runtime = next.endpoint_runtime_snapshot();
    assert_eq!(runtime.tracked_endpoints, 0);
    assert_eq!(runtime.recovery_latency_ms_count, 1);
    assert!(runtime.recovery_latency_ms_sum >= 61_000);
    assert_eq!(
        runtime
            .recovery_latency_ms_buckets
            .last()
            .expect("+Inf bucket")
            .cumulative_count,
        1
    );
}
