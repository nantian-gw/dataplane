#[test]
fn observe_selected_backend_failure_completes_while_snapshot_read_guard_is_held() {
    let snapshot = Snapshot::shared();
    let request = RequestMeta::new(
        Some("api.example.com".to_string()),
        "/",
        "GET",
        BTreeMap::new(),
    );
    {
        let mut current = snapshot.write();
        *current = Snapshot {
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
    }

    let selected = snapshot
        .read()
        .select_backend(&request)
        .expect("selected backend");
    let mut ctx = RequestContext::default();
    cache_selected_backend(&mut ctx, selected, true);

    let held_read = snapshot.read();
    let worker_snapshot = snapshot.clone();
    let (tx, rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut worker_ctx = ctx;
        observe_selected_backend_failure(&worker_snapshot, &mut worker_ctx);
        tx.send(worker_ctx.backend_observation_recorded)
            .expect("send completion");
    });

    assert_eq!(rx.recv_timeout(Duration::from_millis(50)), Ok(true));
    drop(held_read);
    worker.join().expect("worker");
}
