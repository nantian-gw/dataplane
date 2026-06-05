use super::*;

#[test]
fn does_not_fallback_to_service_backend_for_mesh_listener_without_route() {
    let snapshot = Snapshot {
        listeners: vec![mesh_listener("default", "echo", 80, 20080, "HTTP", &[])],
        backends: vec![BackendCluster {
            name: "echo:80".to_string(),
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
        ..Snapshot::default()
    };

    let selected = snapshot.select_backend(&RequestMeta::with_port(
        Some("echo".to_string()),
        20080,
        "/",
        "GET",
        BTreeMap::new(),
    ));

    assert!(selected.is_none());
}

#[test]
fn does_not_fallback_to_service_backend_for_mesh_grpc_listener_without_route() {
    let mut headers = BTreeMap::new();
    headers.insert(
        "content-type".to_string(),
        vec!["application/grpc".to_string()],
    );

    let snapshot = Snapshot {
        listeners: vec![mesh_listener("default", "echo", 7070, 27070, "GRPC", &[])],
        backends: vec![BackendCluster {
            name: "echo:7070".to_string(),
            namespace: "default".to_string(),
            protocol: "GRPC".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.10".to_string(),
                port: 7070,
                healthy: true,
            }],
            wasm_plugin: None,
            ai_service: None,
            token_policy: None,
        }],
        ..Snapshot::default()
    };

    let selected = snapshot.select_backend(&RequestMeta::with_port(
        Some("echo".to_string()),
        27070,
        "/grpcecho.Echo/Ping",
        "POST",
        headers,
    ));

    assert!(selected.is_none());
}

#[test]
fn attached_mesh_route_without_match_does_not_fallback_to_service_backend() {
    let snapshot = Snapshot {
        listeners: vec![mesh_listener(
            "default",
            "echo",
            80,
            20080,
            "HTTP",
            &["default/query-param"],
        )],
        http_routes: vec![HttpRoute {
            name: "query-param".to_string(),
            namespace: "default".to_string(),
            hostnames: vec![],
            parent_refs: vec![ParentRef {
                kind: "Service".to_string(),
                name: "echo".to_string(),
                port: 80,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![aeg_ir::HttpMatch {
                    query_params: vec![aeg_ir::QueryMatch {
                        name: "animal".to_string(),
                        value: "whale".to_string(),
                        match_type: String::new(),
                        ..aeg_ir::QueryMatch::default()
                    }],
                    ..aeg_ir::HttpMatch::default()
                }],
                filters: vec![],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "echo".to_string(),
                    port: 80,
                    ..BackendRef::default()
                }],
                timeouts: None,
                retry: None,
                session_persistence: None,
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "echo:80".to_string(),
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
        ..Snapshot::default()
    };

    let selected = snapshot.select_backend(&RequestMeta::with_port(
        Some("echo.default".to_string()),
        20080,
        "/?animal=dog",
        "GET",
        BTreeMap::new(),
    ));

    assert!(selected.is_none());
}
