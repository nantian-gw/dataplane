#[tokio::test]
async fn reuses_upstream_socket_for_sequential_datagrams() -> Result<()> {
    let upstream = UdpSocket::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream.local_addr()?;
    let downstream = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let gateway_addr = downstream.local_addr()?;
    let client = UdpSocket::bind("127.0.0.1:0").await?;
    let listener = test_listener("default/gw/udp", gateway_addr.port() as u32);
    let snapshot = test_snapshot(listener.clone(), upstream_addr);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server = tokio::spawn(run_with_socket(
        snapshot,
        listener.name.clone(),
        gateway_addr.to_string(),
        Arc::clone(&downstream),
        shutdown_rx,
        disabled_access_log(),
        SharedTrafficStats::shared(),
        ntgw_observability::UdpAdmissionController::new(
            ntgw_observability::UdpAdmissionOptions::default(),
            ntgw_observability::OverloadStats::shared(),
        ),
        UdpSessionStats::shared(),
        Duration::from_millis(20),
    ));

    let upstream_task = tokio::spawn(async move {
        let mut buf = [0; 64];
        let mut peers = Vec::new();
        for expected in [b"first".as_slice(), b"second".as_slice()] {
            let (size, peer) = upstream.recv_from(&mut buf).await?;
            assert_eq!(&buf[..size], expected);
            peers.push(peer);
            upstream.send_to(b"pong", peer).await?;
        }
        Ok::<Vec<std::net::SocketAddr>, anyhow::Error>(peers)
    });

    client.send_to(b"first", gateway_addr).await?;
    let mut buf = [0; 64];
    let (size, _) = client.recv_from(&mut buf).await?;
    assert_eq!(&buf[..size], b"pong");

    client.send_to(b"second", gateway_addr).await?;
    let (size, _) = timeout(Duration::from_millis(200), client.recv_from(&mut buf)).await??;
    assert_eq!(&buf[..size], b"pong");

    let peers = upstream_task.await??;
    assert_eq!(peers.len(), 2);
    assert_eq!(
        peers[0], peers[1],
        "sequential datagrams should reuse the same upstream socket"
    );

    shutdown_tx.send(true).expect("shutdown udp listener");
    server.await??;
    Ok(())
}

#[tokio::test]
async fn queued_datagram_reuses_session_without_waiting_for_response_idle_timeout() -> Result<()> {
    let upstream = UdpSocket::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream.local_addr()?;
    let downstream = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let gateway_addr = downstream.local_addr()?;
    let client = UdpSocket::bind("127.0.0.1:0").await?;
    let listener = test_listener("default/gw/udp", gateway_addr.port() as u32);
    let snapshot = test_snapshot(listener.clone(), upstream_addr);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server = tokio::spawn(run_with_socket(
        snapshot,
        listener.name.clone(),
        gateway_addr.to_string(),
        Arc::clone(&downstream),
        shutdown_rx,
        disabled_access_log(),
        SharedTrafficStats::shared(),
        ntgw_observability::UdpAdmissionController::new(
            ntgw_observability::UdpAdmissionOptions::default(),
            ntgw_observability::OverloadStats::shared(),
        ),
        UdpSessionStats::shared(),
        Duration::from_millis(250),
    ));

    let upstream_task = tokio::spawn(async move {
        let mut buf = [0; 64];
        for expected in [b"first".as_slice(), b"second".as_slice()] {
            let (size, peer) = upstream.recv_from(&mut buf).await?;
            assert_eq!(&buf[..size], expected);
            upstream.send_to(b"pong", peer).await?;
        }
        Ok::<(), anyhow::Error>(())
    });

    client.send_to(b"first", gateway_addr).await?;
    let mut buf = [0; 64];
    let (size, _) = client.recv_from(&mut buf).await?;
    assert_eq!(&buf[..size], b"pong");

    client.send_to(b"second", gateway_addr).await?;
    let (size, _) = timeout(Duration::from_millis(120), client.recv_from(&mut buf)).await??;
    assert_eq!(&buf[..size], b"pong");

    shutdown_tx.send(true).expect("shutdown udp listener");
    upstream_task.await??;
    server.await??;
    Ok(())
}

#[tokio::test]
async fn session_registry_does_not_hold_map_lock_while_initializing_socket() -> Result<()> {
    let registry = UdpSessionRegistry::default();
    let downstream = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let blocked_key = UdpSessionKey {
        listener_name: Arc::from("default/gw/udp"),
        client_addr: "127.0.0.1:40001".parse().expect("client addr"),
        upstream_addr: "127.0.0.1:41001".parse().expect("upstream addr"),
    };
    let independent_key = UdpSessionKey {
        listener_name: Arc::from("default/gw/udp"),
        client_addr: "127.0.0.1:40002".parse().expect("client addr"),
        upstream_addr: "127.0.0.1:41002".parse().expect("upstream addr"),
    };
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();

    let blocked_registry = registry.clone();
    let blocked_downstream = Arc::clone(&downstream);
    let blocked_task = tokio::spawn(async move {
        blocked_registry
            .ensure_sender_with_factory(
                blocked_key,
                blocked_downstream,
                Duration::from_millis(50),
                || async move {
                    let _ = started_tx.send(());
                    let _ = release_rx.await;
                    let upstream = UdpSocket::bind("127.0.0.1:0").await?;
                    upstream.connect("127.0.0.1:41001").await?;
                    Ok::<UdpSocket, anyhow::Error>(upstream)
                },
            )
            .await
    });

    started_rx.await.expect("blocked initializer should start");

    let independent_sender = timeout(
        Duration::from_millis(100),
        registry.ensure_sender_with_factory(
            independent_key,
            Arc::clone(&downstream),
            Duration::from_millis(50),
            || async move {
                let upstream = UdpSocket::bind("127.0.0.1:0").await?;
                upstream.connect("127.0.0.1:41002").await?;
                Ok::<UdpSocket, anyhow::Error>(upstream)
            },
        ),
    )
    .await
    .expect("independent session registration should not block on socket creation")?;

    release_tx.send(()).expect("release blocked initializer");
    let blocked_sender = blocked_task.await??;

    drop(independent_sender);
    drop(blocked_sender);
    Ok(())
}

#[test]
fn session_registry_uses_multiple_shards() {
    let registry = UdpSessionRegistry::default();

    assert!(
        registry.shard_count() > 1,
        "UDP session registry should shard session state to reduce map lock contention"
    );
}

#[test]
fn udp_listener_uses_fixed_dispatcher_instead_of_per_datagram_spawn() {
    let source = include_str!("../../udp.rs");
    let run_with_socket = source
        .split("pub async fn run_with_socket")
        .nth(1)
        .and_then(|tail| tail.split("fn build_udp_session_task").next())
        .expect("run_with_socket source should be present");

    assert!(
        run_with_socket.contains("UdpDatagramDispatcher"),
        "UDP listener should hand accepted datagrams to a fixed dispatcher"
    );
    assert!(
        !run_with_socket.contains("tokio::spawn(async move"),
        "UDP listener should not spawn a new task for every accepted datagram"
    );
}

#[test]
fn build_udp_session_task_skips_access_log_storage_when_disabled() -> Result<()> {
    let listener = test_listener("default/gw/udp", 15000);
    let snapshot = test_snapshot(listener, "127.0.0.1:15001".parse()?);
    let admission =
        UdpAdmissionController::new(UdpAdmissionOptions::default(), OverloadStats::shared());
    let permit = admission
        .try_acquire("default/gw/udp")
        .expect("udp datagram permit");
    let access_log = disabled_access_log();
    let task = build_udp_session_task(
        &snapshot,
        Arc::from("default/gw/udp"),
        "127.0.0.1:25000".parse()?,
        vec![1, 2, 3],
        &access_log,
        SharedTrafficStats::shared(),
        permit,
    )?;

    assert!(task.access_log.is_none());
    assert!(task.access_log_state.is_none());
    Ok(())
}

#[test]
fn build_udp_session_task_reuses_precomputed_listener_label_for_session_key() -> Result<()> {
    let listener = test_listener("default/gw/udp", 15000);
    let snapshot = test_snapshot(listener, "127.0.0.1:15001".parse()?);
    let admission =
        UdpAdmissionController::new(UdpAdmissionOptions::default(), OverloadStats::shared());
    let permit = admission
        .try_acquire("default/gw/udp")
        .expect("udp datagram permit");
    let access_log = disabled_access_log();
    let listener_label = Arc::<str>::from("default/gw/udp");
    let task = build_udp_session_task(
        &snapshot,
        listener_label.clone(),
        "127.0.0.1:25000".parse()?,
        vec![1, 2, 3],
        &access_log,
        SharedTrafficStats::shared(),
        permit,
    )?;

    assert!(Arc::ptr_eq(&task.listener_name, &listener_label));
    assert!(Arc::ptr_eq(&task.session_key().listener_name, &listener_label));
    Ok(())
}

#[tokio::test]
async fn session_registry_tracks_queue_drops_and_idle_evictions() -> Result<()> {
    let stats = UdpSessionStats::shared();
    let registry = UdpSessionRegistry::with_stats(stats.clone());
    let downstream = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    let upstream_server = UdpSocket::bind("127.0.0.1:0").await?;
    let upstream_addr = upstream_server.local_addr()?;
    let key = UdpSessionKey {
        listener_name: Arc::from("default/gw/udp"),
        client_addr: "127.0.0.1:40053".parse().expect("client addr"),
        upstream_addr,
    };
    let listener = test_listener("default/gw/udp", downstream.local_addr()?.port() as u32);
    let snapshot = test_snapshot(listener, upstream_addr);
    let traffic = SharedTrafficStats::shared();
    let admission = UdpAdmissionController::new(
        UdpAdmissionOptions::default(),
        OverloadStats::shared(),
    );
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let upstream_task = tokio::spawn(async move {
        let mut buf = [0; 64];
        for _ in 0..32 {
            let _ = timeout(Duration::from_millis(500), upstream_server.recv_from(&mut buf))
                .await??;
        }
        Ok::<(), anyhow::Error>(())
    });

    let blocked_registry = registry.clone();
    let blocked_downstream = Arc::clone(&downstream);
    let blocked_key = key.clone();
    let blocked_init = tokio::spawn(async move {
        blocked_registry
            .ensure_sender_with_factory(
                blocked_key,
                blocked_downstream,
                Duration::from_millis(1),
                || async move {
                    let _ = started_tx.send(());
                    let _ = release_rx.await;
                    let upstream = UdpSocket::bind("127.0.0.1:0").await?;
                    upstream.connect(upstream_addr).await?;
                    Ok::<UdpSocket, anyhow::Error>(upstream)
                },
            )
            .await
    });

    started_rx.await.expect("blocked initializer should start");
    assert_eq!(stats.snapshot().active_sessions_current, 1);

    let access_log = disabled_access_log();
    for seq in 0..33u8 {
        let permit = admission
            .try_acquire("default/gw/udp")
            .expect("udp datagram permit");
        let task = build_udp_session_task(
            &snapshot,
            Arc::from("default/gw/udp"),
            key.client_addr,
            vec![seq],
            &access_log,
            traffic.clone(),
            permit,
        )?;
        registry
            .dispatch(Arc::clone(&downstream), task, Duration::from_millis(1))
            .await?;
    }

    let queued = stats.snapshot();
    assert_eq!(queued.queue_depth_current, 32);
    assert_eq!(queued.queue_overflow_dropped_total, 1);
    assert_eq!(
        queued
            .queue_depth_by_listener
            .get("default/gw/udp")
            .copied(),
        Some(32)
    );
    assert_eq!(
        queued
            .queue_overflow_dropped_by_listener
            .get("default/gw/udp")
            .copied(),
        Some(1)
    );

    release_tx.send(()).expect("release blocked initializer");
    let sender = blocked_init.await??;
    drop(sender);

    timeout(Duration::from_millis(1_500), async {
        loop {
            let snapshot = stats.snapshot();
            if snapshot.active_sessions_current == 0
                && snapshot.queue_depth_current == 0
                && snapshot.idle_evictions_total == 1
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("idle session should be evicted and metrics should drain");

    let drained = stats.snapshot();
    assert_eq!(
        drained.idle_evictions_by_listener.get("default/gw/udp").copied(),
        Some(1)
    );
    upstream_task.await??;
    Ok(())
}
