#[test]
fn observe_selected_backend_failure_ejects_endpoint_after_threshold() {
    let snapshot = Snapshot::shared();
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );
    {
        snapshot.store(Arc::new(Snapshot {
            http_routes: vec![HttpRoute {
                name: "route".to_string(),
                namespace: "default".to_string(),
                hostnames: vec!["api.example.com".to_string()],
                parent_refs: vec![],
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![],
                    filters: vec![],
                    backend_refs: vec![ntgw_ir::BackendRef {
                        namespace: "default".to_string(),
                        name: "echo".to_string(),
                        port: 8080,
                        ..ntgw_ir::BackendRef::default()
                    }],
                    timeouts: None,
                    retry: None,
                    session_persistence: None,
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            security_policy: None,
            }],
            backends: vec![BackendCluster {
                ai_service: None,
                token_policy: None,
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
            
                security_policy: None,
                circuit_breaker: None,}],
            ..Snapshot::default()
        }));
    }

    let selected = snapshot
        .load()
        .select_backend(&request)
        .expect("selected backend");
    let mut ctx = RequestContext::default();
    cache_selected_backend(&mut ctx, selected, true);

    for _ in 0..PASSIVE_EJECTION_CONSECUTIVE_FAILURES {
        observe_selected_backend_failure(&snapshot, &mut ctx);
        ctx.backend_observation_recorded = false;
    }

    let addresses = collect_selected_addresses(&snapshot.load(), &request, 4);
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
