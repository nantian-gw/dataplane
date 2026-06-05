#[tokio::test]
async fn spawn_bind_task_allows_dual_stack_wildcard_bind_pair() -> Result<()> {
    if StdTcpListener::bind("[::1]:0").is_err() {
        return Ok(());
    }

    let port = free_port();
    let snapshot = Snapshot::shared();
    let app = build_http_app(
        snapshot.clone(),
        HttpRuntimeOptions::default(),
        AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        SessionPersistenceOptions::build(None, None)?,
        SharedTrafficStats::shared(),
        aeg_observability::OverloadStats::shared(),
        HttpCircuitBreakerController::new(Default::default()),
        HttpRateLimitController::new(Default::default()),
        RetryBudgetController::new(Default::default()),
        None,
    )?;
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
