
#[tokio::test]
async fn tls_passthrough_prefers_exact_sni_match_over_wildcard() -> Result<()> {
    let hello = build_client_hello("api.example.com");
    let exact_listener = TcpListener::bind("127.0.0.1:0").await?;
    let exact_addr = exact_listener.local_addr()?;
    let wildcard_listener = TcpListener::bind("127.0.0.1:0").await?;
    let wildcard_addr = wildcard_listener.local_addr()?;
    let gateway_listener = TcpListener::bind("127.0.0.1:0").await?;
    let gateway_addr = gateway_listener.local_addr()?;
    let listener = Listener {
        attached_routes: vec![
            "default/wildcard-route".to_string(),
            "default/exact-route".to_string(),
        ],
        ..test_listener(
            "default/gw/tls",
            gateway_addr.port() as u32,
            "LISTENER_PROTOCOL_TLS_PASSTHROUGH",
        )
    };
    let snapshot = Snapshot::shared();
    snapshot.store(Arc::new(Snapshot {
        listeners: vec![listener.clone()],
        stream_routes: vec![
            StreamRoute {
                name: "wildcard-route".to_string().into(),
                namespace: "default".to_string().into(),
                kind: "ROUTE_KIND_TLS".to_string(),
                parent_refs: Vec::new(),
                rules: vec![StreamRule {
                    name: String::new(),
                    matches: vec![ntgw_ir::StreamMatch {
                        port: gateway_addr.port() as u32,
                        sni_hostname: "*.example.com".to_string().into(),
                        mode: ntgw_ir::TlsRouteMode::default(),
                    }],
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string().into(),
                        name: "wildcard-upstream".to_string().into(),
                        port: wildcard_addr.port() as u32,
                        ..BackendRef::default()
                    }],
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
            StreamRoute {
                name: "exact-route".to_string().into(),
                namespace: "default".to_string().into(),
                kind: "ROUTE_KIND_TLS".to_string(),
                parent_refs: Vec::new(),
                rules: vec![StreamRule {
                    name: String::new(),
                    matches: vec![ntgw_ir::StreamMatch {
                        port: gateway_addr.port() as u32,
                        sni_hostname: "api.example.com".to_string().into(),
                        mode: ntgw_ir::TlsRouteMode::default(),
                    }],
                    backend_refs: vec![BackendRef {
                        namespace: "default".to_string().into(),
                        name: "exact-upstream".to_string().into(),
                        port: exact_addr.port() as u32,
                        ..BackendRef::default()
                    }],
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("wildcard-upstream:{}", wildcard_addr.port()).into(),
                namespace: "default".to_string().into(),
                protocol: "TCP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: wildcard_addr.ip().to_string(),
                    port: wildcard_addr.port() as u32,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("exact-upstream:{}", exact_addr.port()).into(),
                namespace: "default".to_string().into(),
                protocol: "TCP".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: exact_addr.ip().to_string(),
                    port: exact_addr.port() as u32,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    }));
    let expected_hello = hello.clone();
    let exact = tokio::spawn(async move {
        let (mut stream, _) = exact_listener.accept().await?;
        let mut buf = vec![0; expected_hello.len()];
        stream.read_exact(&mut buf).await?;
        assert_eq!(buf, expected_hello);
        stream.write_all(b"exact").await?;
        stream.shutdown().await?;
        Ok::<(), anyhow::Error>(())
    });
    let wildcard = tokio::spawn(async move {
        let accepted = timeout(Duration::from_millis(200), wildcard_listener.accept()).await;
        match accepted {
            Err(_) => Ok::<(), anyhow::Error>(()),
            Ok(Ok((_stream, addr))) => Err(anyhow!(
                "wildcard backend should not receive connection from {addr}"
            )),
            Ok(Err(err)) => Err(anyhow!(err)),
        }
    });

    let server = tokio::spawn(async move {
        let (stream, _) = gateway_listener.accept().await?;
        handle_connection(
            snapshot,
            listener.name,
            stream,
            true,
            disabled_access_log(),
            SharedTrafficStats::shared(),
            TCP_PROXY_BUFFER_BYTES,
            None,
            None,
            std::sync::Arc::new(crate::pool::TcpConnectionPool::new(
                0,
                Duration::from_secs(30),
            )),
        )
        .await
    });

    let mut client = TcpStream::connect(gateway_addr).await?;
    client.write_all(&hello).await?;
    client.shutdown().await?;
    let mut response = Vec::new();
    client.read_to_end(&mut response).await?;

    assert_eq!(response, b"exact");
    exact.await??;
    wildcard.await??;
    server.await??;
    Ok(())
}
