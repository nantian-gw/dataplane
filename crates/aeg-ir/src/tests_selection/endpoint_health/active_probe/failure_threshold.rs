#[test]
fn active_probe_failure_threshold_temporarily_removes_endpoint_from_rotation() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
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
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
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
                ai_service: None,
                token_policy: None,
        }],
        ..Snapshot::default()
    };
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    snapshot.record_endpoint_active_probe_failure(
        selected.backend_name.as_str(),
        &selected.backend,
        2,
    );
    let after_single_failure = collect_http_addresses(&snapshot, &request, 4);
    assert_eq!(
        after_single_failure
            .iter()
            .filter(|address| address.as_str() == "10.0.0.10")
            .count(),
        2
    );

    snapshot.record_endpoint_active_probe_failure(
        selected.backend_name.as_str(),
        &selected.backend,
        2,
    );

    let addresses = collect_http_addresses(&snapshot, &request, 4);
    assert_eq!(
        addresses,
        vec![
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
            "10.0.0.11".to_string(),
        ]
    );
}

#[test]
fn active_probe_flapping_below_threshold_keeps_endpoint_in_rotation() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
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
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
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
                ai_service: None,
                token_policy: None,
        }],
        ..Snapshot::default()
    };
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let selected = snapshot.select_backend(&request).expect("backend");
    snapshot.record_endpoint_active_probe_failure(
        selected.backend_name.as_str(),
        &selected.backend,
        2,
    );
    snapshot
        .record_endpoint_active_probe_success(selected.backend_name.as_str(), &selected.backend);
    snapshot.record_endpoint_active_probe_failure(
        selected.backend_name.as_str(),
        &selected.backend,
        2,
    );

    let addresses = collect_http_addresses(&snapshot, &request, 4);
    assert_eq!(
        addresses
            .iter()
            .filter(|address| address.as_str() == "10.0.0.10")
            .count(),
        2
    );
    assert_eq!(
        addresses
            .iter()
            .filter(|address| address.as_str() == "10.0.0.11")
            .count(),
        2
    );
}

#[test]
fn all_active_unhealthy_endpoints_return_after_probe_recovery() {
    let snapshot = Snapshot {
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
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
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
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
                ai_service: None,
                token_policy: None,
        }],
        ..Snapshot::default()
    };
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );

    let first = snapshot.select_backend(&request).expect("first backend");
    let second = snapshot.select_backend(&request).expect("second backend");
    assert_ne!(first.backend.address, second.backend.address);
    snapshot.record_endpoint_active_probe_failure(first.backend_name.as_str(), &first.backend, 1);
    snapshot.record_endpoint_active_probe_failure(second.backend_name.as_str(), &second.backend, 1);

    let unavailable = snapshot
        .select_http_route(&request)
        .expect("matched route with unhealthy backends");
    assert!(unavailable.backend.is_none());
    assert_eq!(
        unavailable.backend_error,
        Some(crate::BackendSelectionError::NoHealthyBackends)
    );

    snapshot.record_endpoint_active_probe_success(first.backend_name.as_str(), &first.backend);
    let first_recovered = collect_http_addresses(&snapshot, &request, 4);
    assert_eq!(
        first_recovered,
        vec![
            first.backend.address.clone(),
            first.backend.address.clone(),
            first.backend.address.clone(),
            first.backend.address.clone(),
        ]
    );

    snapshot.record_endpoint_active_probe_success(second.backend_name.as_str(), &second.backend);
    let both_recovered = collect_http_addresses(&snapshot, &request, 4);
    assert_eq!(
        both_recovered
            .iter()
            .filter(|address| address.as_str() == first.backend.address.as_str())
            .count(),
        2
    );
    assert_eq!(
        both_recovered
            .iter()
            .filter(|address| address.as_str() == second.backend.address.as_str())
            .count(),
        2
    );
}
