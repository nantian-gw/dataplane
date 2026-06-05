#[test]
fn listener_updates_restart_only_changed_listener() {
    let active = BTreeMap::from([
        (
            "0.0.0.0:80".to_string(),
            PlannedListener {
                name: "default/gw/http".to_string(),
                bind: "0.0.0.0:80".to_string(),
                protocol: ListenerProtocol::Plain,
            },
        ),
        (
            "0.0.0.0:443".to_string(),
            PlannedListener {
                name: "default/gw/https".to_string(),
                bind: "0.0.0.0:443".to_string(),
                protocol: ListenerProtocol::Tls(single_tls_material(
                    "default/example-cert",
                    "CERT-A",
                    "KEY-A",
                    None,
                )),
            },
        ),
    ]);
    let desired = ListenerPlan {
        listeners: vec![
            PlannedListener {
                name: "default/gw/http".to_string(),
                bind: "0.0.0.0:80".to_string(),
                protocol: ListenerProtocol::Plain,
            },
            PlannedListener {
                name: "default/gw/https".to_string(),
                bind: "0.0.0.0:443".to_string(),
                protocol: ListenerProtocol::Tls(single_tls_material(
                    "default/rotated-cert",
                    "CERT-B",
                    "KEY-B",
                    None,
                )),
            },
        ],
    };

    let updates = listener_updates(&active, Some(&desired), &BTreeSet::new());

    assert_eq!(
        updates,
        ListenerUpdatePlan {
            start: vec![desired.listeners[1].clone()],
            stop: vec!["0.0.0.0:443".to_string()],
        }
    );
}

#[test]
fn listener_updates_do_not_restart_when_only_listener_name_changes() {
    let active = BTreeMap::from([
        (
            "0.0.0.0:10080".to_string(),
            PlannedListener {
                name: "mesh/default/echo/10080".to_string(),
                bind: "0.0.0.0:10080".to_string(),
                protocol: ListenerProtocol::Plain,
            },
        ),
        (
            "[::]:10080".to_string(),
            PlannedListener {
                name: "mesh/default/echo/10080".to_string(),
                bind: "[::]:10080".to_string(),
                protocol: ListenerProtocol::Plain,
            },
        ),
    ]);
    let desired = ListenerPlan {
        listeners: vec![
            PlannedListener {
                name: "mesh/default/echo-v2/10080".to_string(),
                bind: "0.0.0.0:10080".to_string(),
                protocol: ListenerProtocol::Plain,
            },
            PlannedListener {
                name: "mesh/default/echo-v2/10080".to_string(),
                bind: "[::]:10080".to_string(),
                protocol: ListenerProtocol::Plain,
            },
        ],
    };

    let updates = listener_updates(&active, Some(&desired), &BTreeSet::new());

    assert_eq!(updates, ListenerUpdatePlan::default());
}

#[test]
fn listener_updates_restart_finished_listener_without_touching_others() {
    let listener = PlannedListener {
        name: "default/gw/http".to_string(),
        bind: "127.0.0.1:8080".to_string(),
        protocol: ListenerProtocol::Plain,
    };
    let active = BTreeMap::from([(listener.bind.clone(), listener.clone())]);
    let desired = ListenerPlan {
        listeners: vec![listener.clone()],
    };
    let finished = BTreeSet::from([listener.bind.clone()]);

    let updates = listener_updates(&active, Some(&desired), &finished);

    assert_eq!(
        updates,
        ListenerUpdatePlan {
            start: vec![listener],
            stop: vec!["127.0.0.1:8080".to_string()],
        }
    );
}

#[test]
fn listener_updates_force_reload_restarts_active_listeners_even_when_plan_is_unchanged() {
    let listener = PlannedListener {
        name: "default/gw/http".to_string(),
        bind: "127.0.0.1:8080".to_string(),
        protocol: ListenerProtocol::Plain,
    };
    let active = BTreeMap::from([(listener.bind.clone(), listener.clone())]);
    let desired = ListenerPlan {
        listeners: vec![listener.clone()],
    };

    let updates =
        listener_updates_with_force_reload(&active, Some(&desired), &BTreeSet::new(), true);

    assert_eq!(
        updates,
        ListenerUpdatePlan {
            start: vec![listener],
            stop: vec!["127.0.0.1:8080".to_string()],
        }
    );
}

#[test]
fn listener_replace_reports_retained_listeners_when_plan_is_unchanged() -> anyhow::Result<()> {
    let snapshot = Snapshot::shared();
    let runtime_stats = RuntimeStats::shared();
    let traffic = SharedTrafficStats::shared();
    let overload = OverloadStats::shared();
    let asset_root = std::env::temp_dir()
        .join("aether-gateway")
        .join("listener-replace-reports-retained")
        .join(unique_asset_dir_name());
    fs::create_dir_all(&asset_root)?;

    let bind = format!("127.0.0.1:{}", free_tcp_port());
    let listener = PlannedListener {
        name: "default/gw/http".to_string(),
        bind: bind.clone(),
        protocol: ListenerProtocol::Plain,
    };
    let plan = ListenerPlan {
        listeners: vec![listener.clone()],
    };
    let mut listeners = ListenerSet::default();

    let initial = listeners.replace(
        Some(plan.clone()),
        listener_replace_context(
            "v1",
            snapshot.clone(),
            &runtime_stats,
            traffic.clone(),
            overload.clone(),
            &asset_root,
        )?,
    );
    assert!(
        initial.first_error.is_none(),
        "initial listener should start: {:?}",
        initial.first_error
    );
    wait_for_std_listener(&bind)?;

    let retained = listeners.replace(
        Some(plan),
        listener_replace_context("v2", snapshot, &runtime_stats, traffic, overload, &asset_root)?,
    );

    assert!(retained.failures.is_empty());
    assert!(retained.started_listeners.is_empty());
    assert_eq!(retained.retained_listeners, vec!["default/gw/http"]);

    listeners.shutdown_all();
    let _ = fs::remove_dir_all(&asset_root);
    Ok(())
}

#[test]
fn listener_replace_retains_last_good_listener_when_new_listener_start_fails() -> anyhow::Result<()>
{
    let snapshot = Snapshot::shared();
    let runtime_stats = RuntimeStats::shared();
    let traffic = SharedTrafficStats::shared();
    let overload = OverloadStats::shared();
    let asset_root = std::env::temp_dir()
        .join("aether-gateway")
        .join("listener-replace-retains-last-good")
        .join(unique_asset_dir_name());
    fs::create_dir_all(&asset_root)?;

    let old_bind = format!("127.0.0.1:{}", free_tcp_port());
    let new_bind = format!("127.0.0.1:{}", free_tcp_port());
    let old_listener = PlannedListener {
        name: "default/gw/http".to_string(),
        bind: old_bind.clone(),
        protocol: ListenerProtocol::Plain,
    };
    let mut listeners = ListenerSet::default();

    let initial = listeners.replace(
        Some(ListenerPlan {
            listeners: vec![old_listener.clone()],
        }),
        listener_replace_context(
            "v1",
            snapshot.clone(),
            &runtime_stats,
            traffic.clone(),
            overload.clone(),
            &asset_root,
        )?,
    );
    assert!(
        initial.first_error.is_none(),
        "initial listener should start: {:?}",
        initial.first_error
    );
    assert_eq!(
        listeners
            .active_bind_plan()
            .expect("active listener")
            .listeners,
        vec![old_listener.clone()]
    );
    wait_for_std_listener(&old_bind)?;

    fs::remove_dir_all(&asset_root)?;
    fs::write(&asset_root, b"not a directory")?;
    let failed = listeners.replace(
        Some(ListenerPlan {
            listeners: vec![PlannedListener {
                name: "default/gw/https".to_string(),
                bind: new_bind,
                protocol: ListenerProtocol::Tls(single_tls_material(
                    "default/example-cert",
                    VALID_SERVER_CERT_PEM,
                    VALID_SERVER_KEY_PEM,
                    None,
                )),
            }],
        }),
        listener_replace_context(
            "v2",
            snapshot,
            &runtime_stats,
            traffic,
            overload,
            &asset_root,
        )?,
    );

    assert!(failed.retry_start);
    assert!(
        failed
            .first_error
            .as_deref()
            .is_some_and(|err| err.contains("File exists",)
                || err.contains("Not a directory")
                || err.contains("not a directory")),
        "unexpected listener start failure: {:?}",
        failed.first_error
    );
    assert_eq!(
        listeners
            .active_bind_plan()
            .expect("last-good listener should remain active")
            .listeners,
        vec![old_listener]
    );
    std::net::TcpStream::connect(&old_bind).context("connect retained last-good listener")?;

    listeners.shutdown_all();
    let _ = fs::remove_file(&asset_root);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn listener_replace_keeps_existing_http_connection_when_new_listener_start_fails(
) -> anyhow::Result<()> {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("upstream bind")?;
    let upstream_addr = upstream_listener.local_addr().context("upstream addr")?;
    let old_port = free_tcp_port();
    let old_bind = format!("127.0.0.1:{old_port}");
    let new_bind = format!("127.0.0.1:{}", free_tcp_port());
    let snapshot = simple_http_snapshot(
        old_port,
        "/during-reload",
        upstream_addr.port() as u32,
        "HTTP",
    );
    let runtime_stats = RuntimeStats::shared();
    let traffic = SharedTrafficStats::shared();
    let overload = OverloadStats::shared();
    let asset_root = std::env::temp_dir()
        .join("aether-gateway")
        .join("listener-replace-keeps-existing-http-connection")
        .join(unique_asset_dir_name());
    fs::create_dir_all(&asset_root)?;

    let old_listener = PlannedListener {
        name: "default/gw/http".to_string(),
        bind: old_bind.clone(),
        protocol: ListenerProtocol::Plain,
    };
    let mut listeners = ListenerSet::default();

    let initial = listeners.replace(
        Some(ListenerPlan {
            listeners: vec![old_listener.clone()],
        }),
        listener_replace_context(
            "v1",
            snapshot.clone(),
            &runtime_stats,
            traffic.clone(),
            overload.clone(),
            &asset_root,
        )?,
    );
    assert!(
        initial.first_error.is_none(),
        "initial listener should start: {:?}",
        initial.first_error
    );
    wait_for_listener(old_port).await;

    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(
            request.starts_with("GET /during-reload HTTP/1.1\r\n"),
            "unexpected request: {request}"
        );
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nretained")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    let mut client = TcpStream::connect(("127.0.0.1", old_port))
        .await
        .context("connect last-good listener before rejected snapshot")?;

    fs::remove_dir_all(&asset_root)?;
    fs::write(&asset_root, b"not a directory")?;
    let failed = listeners.replace(
        Some(ListenerPlan {
            listeners: vec![PlannedListener {
                name: "default/gw/https".to_string(),
                bind: new_bind,
                protocol: ListenerProtocol::Tls(single_tls_material(
                    "default/example-cert",
                    VALID_SERVER_CERT_PEM,
                    VALID_SERVER_KEY_PEM,
                    None,
                )),
            }],
        }),
        listener_replace_context(
            "v2",
            snapshot,
            &runtime_stats,
            traffic,
            overload,
            &asset_root,
        )?,
    );

    assert!(failed.retry_start);
    client
        .write_all(b"GET /during-reload HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
        .await?;
    let response = read_http_response(&mut client)
        .await
        .context("read response through retained existing connection")?;
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "unexpected response: {response}"
    );
    assert!(
        response.ends_with("\r\n\r\nretained"),
        "unexpected response: {response}"
    );

    upstream.await??;
    listeners.shutdown_all();
    let _ = fs::remove_file(&asset_root);
    Ok(())
}

fn listener_replace_context<'a>(
    version: &'a str,
    snapshot: aeg_ir::SharedSnapshot,
    runtime_stats: &'a aeg_observability::SharedRuntimeStats,
    traffic: SharedTrafficStats,
    overload: aeg_observability::SharedOverloadStats,
    asset_root: &'a std::path::Path,
) -> anyhow::Result<ListenerReplaceContext<'a>> {
    Ok(ListenerReplaceContext {
        version,
        snapshot,
        runtime: RuntimeOptions::default(),
        access_log: AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        session_persistence: SessionPersistenceOptions::build(None, None)?,
        runtime_stats,
        traffic,
        admission: aeg_observability::HttpAdmissionController::new(Default::default(), overload),
        circuit_breaker: HttpCircuitBreakerController::new(Default::default()),
        rate_limit: HttpRateLimitController::new(Default::default()),
        retry_budget: RetryBudgetController::new(Default::default()),
        asset_root,
        force_reload: false,
        stage_recorder: None,
    })
}

fn wait_for_std_listener(bind: &str) -> anyhow::Result<()> {
    for _ in 0..50 {
        if std::net::TcpStream::connect(bind).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(anyhow!("listener {bind} did not become ready"))
}
