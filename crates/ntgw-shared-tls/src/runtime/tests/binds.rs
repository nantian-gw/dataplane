#[tokio::test]
async fn spawn_bind_task_allows_dual_stack_wildcard_bind_pair() -> Result<()> {
    if StdTcpListener::bind("[::1]:0").is_err() {
        return Ok(());
    }

    let port = free_port();
    let snapshot = Snapshot::shared();
    let opts = GatewayProxyOptions {
        snapshot: snapshot.clone(),
        access_log: AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        session_persistence: SessionPersistenceOptions::build(None, None)?,
        traffic: SharedTrafficStats::shared(),
        admission: HttpAdmissionController::new(
            HttpAdmissionOptions::default(),
            OverloadStats::shared(),
        ),
        circuit_breaker: HttpCircuitBreakerController::new(Default::default()),
        rate_limit: HttpRateLimitController::new(Default::default()),
        retry_budget: RetryBudgetController::new(Default::default()),
        downstream_read_timeout: None,
        downstream_max_connection_age: None,
        upstream_tcp_keepalive: None,
        upstream_tuning: Default::default(),
        request_tracing_enabled: false,
        max_request_body_bytes: 0,
        max_request_header_bytes: 0,
        ai_gateway_max_request_body_bytes: 0,
        listener_name_hint: None,
        listener_port_hint: None,
        cache: CacheManager::new(CacheOptions {
            enabled: false,
            max_size_bytes: 0,
            max_entry_size_bytes: 0,
            default_ttl: Duration::from_secs(0),
        }),
        wasm_filter: None,
        ai_filter: None,
    };
    let app = build_http_app(opts, HttpRuntimeOptions::default())?;
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let ipv4_bind = PlannedSharedTlsBind {
        bind: format!("0.0.0.0:{port}"),
        terminate: None,
        passthrough: None,
    };
    let ipv6_bind = PlannedSharedTlsBind {
        bind: format!("[::]:{port}"),
        terminate: None,
        passthrough: None,
    };

    let ipv4_task = spawn_bind_task(
        ipv4_bind.clone(),
        snapshot.clone(),
        app.clone(),
        shutdown_rx.clone(),
    )
    .await?;
    let ipv6_task = spawn_bind_task(ipv6_bind.clone(), snapshot, app, shutdown_rx).await?;

    stop_bind_task(ipv6_bind.bind.as_str(), ipv6_task).await;
    stop_bind_task(ipv4_bind.bind.as_str(), ipv4_task).await;
    Ok(())
}

#[tokio::test]
async fn bind_listener_marks_ipv6_wildcard_as_v6_only() -> Result<()> {
    if StdTcpListener::bind("[::1]:0").is_err() {
        return Ok(());
    }

    let port = free_port();
    let ipv4_listener = bind_tcp_listener(&format!("0.0.0.0:{port}")).await?;
    let ipv6_listener = bind_tcp_listener(&format!("[::]:{port}")).await?;

    drop(ipv6_listener);
    drop(ipv4_listener);
    Ok(())
}
