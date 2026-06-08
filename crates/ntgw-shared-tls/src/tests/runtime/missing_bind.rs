use super::*;

#[derive(Debug, Default)]
struct RecordingStageRecorder {
    stages: Mutex<Vec<String>>,
}

impl RecordingStageRecorder {
    fn has_stage(&self, stage: &str) -> bool {
        self.stages
            .lock()
            .expect("stage recorder")
            .iter()
            .any(|item| item == stage)
    }
}

impl ApplyStageRecorder for RecordingStageRecorder {
    fn observe_apply_stage_duration(&self, stage: &str, _duration_ms: u64) {
        self.stages
            .lock()
            .expect("stage recorder")
            .push(stage.to_string());
    }
}

#[tokio::test]
async fn shared_tls_runtime_marks_version_applied_when_https_listener_has_no_bind_plan()
-> Result<()> {
    let snapshot = Snapshot::shared();
    *snapshot.write() = Snapshot {
        id: "v-missing-identity".to_string(),
        listeners: vec![Listener {
            name: "default/gw/https".to_string(),
            address: "127.0.0.1".to_string(),
            port: free_tcp_port().into(),
            protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
            tls: Some(TlsConfig {
                enabled: true,
                passthrough: false,
                secret_refs: vec!["default/missing-cert".to_string()],
                sni_hosts: vec![],
                min_version: "1.2".to_string(),
                max_version: "1.3".to_string(),
                frontend_validation: None,
            }),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };

    let updates = SnapshotSignal::shared();
    let runtime_stats = RuntimeStats::shared();
    let (_config_tx, config_rx) = watch::channel(Arc::new(ReloadableRuntimeConfig {
        runtime: RuntimeOptions {
            reload_retry_interval: Duration::from_millis(10),
            ..RuntimeOptions::default()
        },
        http: ntgw_http::ReloadableRuntimeConfig {
            runtime: HttpRuntimeOptions::default(),
            access_log: AccessLogOptions {
                enabled: false,
                ..AccessLogOptions::default()
            },
            session_persistence: SessionPersistenceOptions::build(None, None)?,
        },
        stream: ntgw_stream::ReloadableRuntimeConfig {
            runtime: ntgw_stream::RuntimeOptions::default(),
            access_log: AccessLogOptions {
                enabled: false,
                ..AccessLogOptions::default()
            },
        },
    }));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let recorder = Arc::new(RecordingStageRecorder::default());
    let stage_recorder: SharedApplyStageRecorder = recorder.clone();

    let task = tokio::spawn(run(
        snapshot,
        updates,
        config_rx,
        runtime_stats.clone(),
        SharedTrafficStats::shared(),
        ntgw_observability::OverloadStats::shared(),
        Arc::new(std::sync::RwLock::new(HttpCircuitBreakerController::new(
            Default::default(),
        ))),
        Arc::new(std::sync::RwLock::new(HttpRateLimitController::new(
            Default::default(),
        ))),
        Arc::new(std::sync::RwLock::new(RetryBudgetController::new(
            Default::default(),
        ))),
        Some(stage_recorder),
        shutdown_rx,
    ));

    timeout(Duration::from_millis(250), async {
        loop {
            if runtime_stats.snapshot().tls_last_good_reload_version == "v-missing-identity" {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert!(recorder.has_stage("listener_plan"));

    let _ = shutdown_tx.send(true);
    task.await??;
    Ok(())
}
