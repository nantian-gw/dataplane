use super::*;

#[tokio::test]
async fn run_marks_unchanged_stream_plan_as_applied_for_new_version() -> Result<()> {
    let snapshot = ntgw_ir::Snapshot::shared();
    let updates = ntgw_ir::SnapshotSignal::shared();
    let runtime_stats = RuntimeStats::shared();
    let overload = ntgw_observability::OverloadStats::shared();
    let listener = Listener {
        name: "default/gw/tcp".to_string(),
        address: "127.0.0.1".to_string(),
        port: 0,
        protocol: "LISTENER_PROTOCOL_TCP".to_string(),
        attached_routes: vec!["default/route-a".to_string()],
        ..Listener::default()
    };
    *snapshot.write() = ntgw_ir::Snapshot {
        id: "v1".to_string(),
        listeners: vec![listener.clone()],
        ..ntgw_ir::Snapshot::default()
    };

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (_config_tx, config_rx) = watch::channel(Arc::new(ReloadableRuntimeConfig {
        runtime: RuntimeOptions {
            reload_retry_interval: Duration::from_millis(20),
            ..RuntimeOptions::default()
        },
        access_log: AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
    }));
    let runtime_task = tokio::spawn(run(
        snapshot.clone(),
        updates.clone(),
        config_rx,
        runtime_stats.clone(),
        SharedTrafficStats::shared(),
        ntgw_observability::UdpSessionStats::shared(),
        overload,
        shutdown_rx,
    ));

    let initial_apply = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if runtime_stats.snapshot().stream_last_good_reload_version == "v1" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        initial_apply.is_ok(),
        "initial stream listener should apply"
    );

    let mut unchanged_topology = listener.clone();
    unchanged_topology.attached_routes = vec!["default/route-b".to_string()];
    let previous = snapshot.read().clone();
    let mut next = ntgw_ir::Snapshot {
        id: "v2".to_string(),
        listeners: vec![unchanged_topology],
        ..ntgw_ir::Snapshot::default()
    };
    next.inherit_runtime_state_from(&previous);
    *snapshot.write() = next;
    updates.notify_changed();

    let unchanged_plan_apply = tokio::time::timeout(Duration::from_millis(250), async {
        loop {
            if runtime_stats.snapshot().stream_last_good_reload_version == "v2" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;

    let _ = shutdown_tx.send(true);
    runtime_task.await??;

    assert!(
        unchanged_plan_apply.is_ok(),
        "stream runtime should mark an unchanged listener plan as applied for the new snapshot version"
    );
    Ok(())
}
