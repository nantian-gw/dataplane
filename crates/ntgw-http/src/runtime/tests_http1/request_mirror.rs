fn mirrored_http_snapshot(
    listener_port: u16,
    path: &str,
    primary_port: u32,
    mirror_port: u32,
) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/route".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                namespace: "default".to_string(),
                name: "gw".to_string(),
                section_name: String::new(),
                port: listener_port as u32,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: path.to_string(),
                    path_type: "Exact".to_string(),
                    ..HttpMatch::default()
                }],
                filters: vec![Filter {
                    filter_type: "RequestMirror".to_string(),
                    request_mirror: Some(ntgw_ir::RequestMirrorFilter {
                        backend_ref: BackendRef {
                            namespace: "default".to_string(),
                            name: "mirror".to_string(),
                            port: mirror_port,
                            ..BackendRef::default()
                        },
                        ..ntgw_ir::RequestMirrorFilter::default()
                    }),
                    ..Filter::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "primary".to_string(),
                    port: primary_port,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("primary:{primary_port}"),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: primary_port,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("mirror:{mirror_port}"),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: mirror_port,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));
    shared
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_mirror_filter_sends_get_to_mirror_backend() {
    install_rustls_provider();
    let primary_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("primary bind");
    let primary_addr = primary_listener.local_addr().expect("primary addr");
    let mirror_listener = TcpListener::bind("127.0.0.1:0").await.expect("mirror bind");
    let mirror_addr = mirror_listener.local_addr().expect("mirror addr");
    let gateway_port = free_tcp_port();
    let snapshot = mirrored_http_snapshot(
        gateway_port,
        "/mirror",
        primary_addr.port() as u32,
        mirror_addr.port() as u32,
    );
    let runtime = RuntimeOptions {
        enable_ipv6: false,
        ..RuntimeOptions::default()
    };
    let plan = build_listener_plan(&snapshot.load(), &runtime, None).expect("plan");
    let server = start_server(
        plan,
        snapshot.clone(),
        runtime,
        AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None).expect("session options"),
        SharedTrafficStats::shared(),
    )
    .expect("start server");

    let primary = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), primary_listener.accept())
            .await
            .context("primary accept timeout")??;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /mirror HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });
    let mirror = tokio::spawn(async move {
        let (mut stream, _) = timeout(Duration::from_secs(2), mirror_listener.accept())
            .await
            .context("mirror accept timeout")??;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET /mirror HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\n\r\nmirror")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    wait_for_listener(gateway_port).await;

    let result = async {
        let mut client = TcpStream::connect(("127.0.0.1", gateway_port)).await?;
        client
            .write_all(b"GET /mirror HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
            .await?;
        let response = read_http_response(&mut client).await?;
        assert!(response.starts_with("HTTP/1.1 200"));
        assert!(response.ends_with("\r\n\r\nok"));
        Ok::<(), anyhow::Error>(())
    }
    .await;

    stop_server(server);
    result.expect("client flow");
    primary
        .await
        .expect("primary task")
        .expect("primary result");
    mirror.await.expect("mirror task").expect("mirror result");
}
