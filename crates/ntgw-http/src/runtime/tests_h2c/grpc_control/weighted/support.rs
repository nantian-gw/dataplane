fn weighted_grpc_h2c_snapshot(
    listener_port: u16,
    backend_a_port: u32,
    backend_b_port: u32,
) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            attached_routes: vec!["default/grpc-route".to_string()],
            ..Listener::default()
        }],
        security_policy: None,
        grpc_routes: vec![GrpcRoute {
            name: "grpc-route".to_string(),
            namespace: "default".to_string(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                namespace: "default".to_string(),
                name: "gw".to_string(),
                section_name: String::new(),
                port: listener_port as u32,
                ..ParentRef::default()
            }],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![GrpcMatch {
                    service: "helloworld.Greeter".to_string(),
                    method: "Watch".to_string(),
                    ..GrpcMatch::default()
                }],
                backend_refs: vec![
                    BackendRef {
                        namespace: "default".to_string(),
                        name: "grpc-backend-a".to_string(),
                        port: backend_a_port,
                        weight: 1,
                        ..BackendRef::default()
                    },
                    BackendRef {
                        namespace: "default".to_string(),
                        name: "grpc-backend-b".to_string(),
                        port: backend_b_port,
                        weight: 1,
                        ..BackendRef::default()
                    },
                ],
                ..GrpcRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        security_policy: None,
        }],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("grpc-backend-a:{backend_a_port}").into(),
                namespace: "default".to_string().into(),
                protocol: "GRPC".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: backend_a_port,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                security_policy: None,
                circuit_breaker: None,},
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("grpc-backend-b:{backend_b_port}").into(),
                namespace: "default".to_string().into(),
                protocol: "GRPC".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: backend_b_port,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                security_policy: None,
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    }));

    let mut s = (**shared.load()).clone();

    s.rebuild_runtime_indexes();

    shared.store(Arc::new(s));
    shared
}

fn weighted_mesh_grpc_h2c_snapshot(
    listener_port: u16,
    backend_a_port: u32,
    backend_b_port: u32,
) -> ntgw_ir::SharedSnapshot {
    weighted_mesh_grpc_h2c_snapshot_with_addresses(
        listener_port,
        ("127.0.0.1", backend_a_port),
        ("127.0.0.1", backend_b_port),
    )
}

fn weighted_mesh_grpc_h2c_snapshot_with_addresses(
    listener_port: u16,
    backend_a: (&str, u32),
    backend_b: (&str, u32),
) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![Listener {
            name: format!("mesh/default/echo/{listener_port}"),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "GRPC".to_string(),
            attached_routes: vec!["default/mesh-grpc-route".to_string()],
            metadata: BTreeMap::from([
                (
                    "nantian.dev/frontend-kind".to_string(),
                    "Service".to_string(),
                ),
                (
                    "nantian.dev/frontend-namespace".to_string(),
                    "default".to_string(),
                ),
                ("nantian.dev/frontend-name".to_string(), "echo".to_string()),
                ("nantian.dev/frontend-port".to_string(), "7070".to_string()),
            ]),
            ..Listener::default()
        }],
        security_policy: None,
        grpc_routes: vec![GrpcRoute {
            name: "mesh-grpc-route".to_string(),
            namespace: "default".to_string(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                kind: "Service".to_string(),
                name: "echo".to_string(),
                port: 7070,
                ..ParentRef::default()
            }],
            rules: vec![GrpcRule {
                name: String::new(),
                matches: vec![],
                backend_refs: vec![
                    BackendRef {
                        namespace: "default".to_string(),
                        name: "echo-v1".to_string(),
                        port: backend_a.1,
                        weight: 1,
                        ..BackendRef::default()
                    },
                    BackendRef {
                        namespace: "default".to_string(),
                        name: "echo-v2".to_string(),
                        port: backend_b.1,
                        weight: 1,
                        ..BackendRef::default()
                    },
                ],
                ..GrpcRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        security_policy: None,
        }],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("echo-v1:{}", backend_a.1).into(),
                namespace: "default".to_string().into(),
                protocol: "GRPC".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: backend_a.0.to_string(),
                    port: backend_a.1,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                security_policy: None,
                circuit_breaker: None,},
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("echo-v2:{}", backend_b.1).into(),
                namespace: "default".to_string().into(),
                protocol: "GRPC".to_string().into(),
                endpoints: vec![BackendEndpoint {
                    address: backend_b.0.to_string(),
                    port: backend_b.1,
                    healthy: true,
                }],
                wasm_plugin: None,
            
                security_policy: None,
                circuit_breaker: None,},
        ],
        ..Snapshot::default()
    }));

    let mut s = (**shared.load()).clone();

    s.rebuild_runtime_indexes();

    shared.store(Arc::new(s));
    shared
}

async fn serve_grpc_backend_connections(
    listener: TcpListener,
    backend_id: &'static str,
    seen_tx: mpsc::UnboundedSender<&'static str>,
) -> anyhow::Result<()> {
    let mut tasks = Vec::new();
    loop {
        let accepted = timeout(Duration::from_millis(500), listener.accept()).await;
        let Ok(Ok((socket, _))) = accepted else {
            break;
        };
        let backend_seen_tx = seen_tx.clone();
        tasks.push(tokio::spawn(async move {
            let mut connection = h2server::handshake(socket).await?;
            loop {
                let accepted = timeout(Duration::from_millis(500), connection.accept()).await;
                let Ok(stream) = accepted else {
                    break;
                };
                let Some(stream) = stream else {
                    break;
                };
                let (request, mut respond) = stream.context("accept grpc request stream")?;
                assert_eq!(request.uri().path(), "/helloworld.Greeter/Watch");
                backend_seen_tx
                    .send(backend_id)
                    .map_err(|_| anyhow!("seen channel closed"))?;
                let response = Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "application/grpc")
                    .body(())?;
                let mut send = respond.send_response(response, false)?;
                let mut trailers = HeaderMap::new();
                trailers.insert("grpc-status", "0".parse()?);
                send.send_trailers(trailers)?;
            }
            Ok::<(), anyhow::Error>(())
        }));
    }

    for task in tasks {
        task.await
            .expect("grpc backend connection task")
            .expect("grpc backend connection result");
    }

    Ok(())
}
