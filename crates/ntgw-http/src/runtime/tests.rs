use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::TcpListener as StdTcpListener,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, anyhow};
use h2::{client as h2client, server as h2server};
use http::{HeaderMap, Request, Response, StatusCode};
use ntgw_ir::SnapshotSignal;
use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendPolicy, BackendRef, CorsFilter, DirectResponseFilter,
    ExtensionFilter, Filter, GrpcMatch, GrpcRoute, GrpcRule, HeaderModifier, HeaderOperation,
    HttpMatch, HttpRoute, HttpRule, Listener, ParentRef, RetryPolicy, SecretMaterial, Snapshot,
    TlsConfig,
};
use ntgw_observability::{
    AccessLogOptions, ApplyStageRecorder, HttpAdmissionController, HttpCircuitBreakerController,
    HttpRateLimitController, OverloadStats, RetryBudgetController, RuntimeStats,
    SharedApplyStageRecorder, SharedTrafficStats, TrafficSnapshot, shutdown_access_log_writer,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    time::{Duration, sleep, timeout},
};

use super::listener_plan::{bind_addrs, unique_asset_dir_name};
use super::listener_set::ListenerReplaceContext;
use super::plan::active_listener_binds_for_plan_build;
use super::server::server_conf_for_runtime;
use super::updates::ListenerUpdatePlan;
use super::{
    LISTENER_ADDRESSES_METADATA_KEY, ListenerPlan, ListenerProtocol, ListenerSet, PlannedListener,
    ReloadableRuntimeConfig, RuntimeListenerProtocol, RuntimeOptions, build_http_app,
    build_listener_plan, build_listener_plan_with_bind_checker,
    build_listener_plan_with_bind_checker_for_runtime, http3_available, listener_port_hint,
    listener_updates, listener_updates_with_force_reload, materialize_runtime_plan,
    plain_http_server_options, process_accepted_stream, should_suppress_unavailable_bind_warning,
    start_server, start_server_with_overload_stats, stop_server, tcp_socket_options_for_bind,
};
use crate::cache::{CacheManager, CacheOptions};
use crate::proxy::GatewayProxyOptions;
use crate::session::SessionPersistenceOptions;
use pingora::{
    protocols::tls::SslStream,
    tls::ssl::{SslConnector, SslMethod, SslVerifyMode},
};

const VALID_SERVER_CERT_PEM: &str = include_str!("../../../../testdata/backendtls/server-san.crt");
const VALID_SERVER_KEY_PEM: &str = include_str!("../../../../testdata/backendtls/server-san.key");

include!("tests_tls_assets.rs");

include!("tests_bench.rs");

include!("tests_capacity.rs");

include!("tests_listener_plan_core.rs");

include!("tests_listener_plan_updates.rs");

include!("tests_websocket.rs");

include!("tests_http1.rs");

include!("tests_h2c.rs");

include!("tests_support_snapshots.rs");

include!("tests_support_io.rs");

include!("tests_support_helpers.rs");

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

#[test]
fn http_runtime_records_listener_plan_and_tls_asset_reload_stages() -> anyhow::Result<()> {
    let snapshot = Snapshot::shared();
    snapshot.store(Arc::new(Snapshot {
        id: "v1".to_string(),
        listeners: vec![Listener {
            name: "default/gw/http".to_string(),
            address: "127.0.0.1".to_string(),
            port: free_tcp_port() as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    }));
    let updates = SnapshotSignal::shared();
    let (_config_tx, config_rx) = tokio::sync::watch::channel(Arc::new(ReloadableRuntimeConfig {
        runtime: RuntimeOptions {
            reload_retry_interval: Duration::from_millis(10),
            ..RuntimeOptions::default()
        },
        access_log: AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        session_persistence: SessionPersistenceOptions::build(None, None)?,
    }));
    let recorder = Arc::new(RecordingStageRecorder::default());
    let stage_recorder: SharedApplyStageRecorder = recorder.clone();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let handle = super::spawn(
        snapshot,
        updates,
        config_rx,
        RuntimeStats::shared(),
        SharedTrafficStats::shared(),
        OverloadStats::shared(),
        Arc::new(parking_lot::RwLock::new(HttpCircuitBreakerController::new(
            Default::default(),
        ))),
        Arc::new(parking_lot::RwLock::new(HttpRateLimitController::new(
            Default::default(),
        ))),
        Arc::new(parking_lot::RwLock::new(RetryBudgetController::new(
            Default::default(),
        ))),
        Some(stage_recorder),
        shutdown_rx,
    )?;

    wait_for_stage(&recorder, "listener_plan");
    wait_for_stage(&recorder, "tls_assets");
    shutdown_tx.send(true).expect("shutdown runtime");
    handle
        .join()
        .map_err(|_| anyhow!("http runtime panicked"))?;
    Ok(())
}

fn wait_for_stage(recorder: &RecordingStageRecorder, stage: &str) {
    for _ in 0..200 {
        if recorder.has_stage(stage) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("expected stage {stage} to be recorded");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_http_stream_processes_basic_h1_request() -> anyhow::Result<()> {
    install_rustls_provider();
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("upstream bind")?;
    let upstream_addr = upstream_listener.local_addr().context("upstream addr")?;
    let snapshot = simple_http_snapshot(free_tcp_port(), "/", upstream_addr.port() as u32, "HTTP");
    let traffic = SharedTrafficStats::shared();
    let admission = HttpAdmissionController::new(
        RuntimeOptions::default().admission.clone(),
        ntgw_observability::OverloadStats::shared(),
    );
    let opts = GatewayProxyOptions {
        snapshot,
        access_log: AccessLogOptions {
            enabled: false,
            ..AccessLogOptions::default()
        },
        session_persistence: SessionPersistenceOptions::build(None, None)?,
        traffic: traffic.clone(),
        admission,
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
    let app = build_http_app(opts, RuntimeOptions::default())?;

    let (client_io, server_io) = tokio::io::duplex(16 * 1024);
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(process_accepted_stream(
        app,
        Box::new(server_io),
        shutdown_rx,
    ));
    let upstream = tokio::spawn(async move {
        let (mut stream, _) = upstream_listener.accept().await?;
        let request = read_http_headers(&mut stream).await?;
        assert!(request.starts_with("GET / HTTP/1.1\r\n"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .await?;
        Ok::<(), anyhow::Error>(())
    });

    let mut client = client_io;
    client
        .write_all(b"GET / HTTP/1.1\r\nHost: example.com\r\n\r\n")
        .await?;

    let mut response = Vec::new();
    loop {
        let mut buf = [0; 1024];
        let read = timeout(Duration::from_secs(2), client.read(&mut buf)).await??;
        if read == 0 {
            break;
        }
        response.extend_from_slice(&buf[..read]);
        if let Some(headers_end) = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|index| index + 4)
        {
            let headers = String::from_utf8_lossy(&response[..headers_end]);
            let content_length = header_value(&headers, "content-length")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or_default();
            if response.len() >= headers_end + content_length {
                break;
            }
        }
    }
    let response = String::from_utf8(response)?;
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "unexpected response: {response}"
    );
    assert!(
        response.ends_with("\r\n\r\nok"),
        "unexpected response: {response}"
    );

    drop(client);
    let _ = shutdown_tx.send(true);
    task.await??;
    upstream.await??;
    let stats = wait_for_traffic_snapshot(&traffic, |stats| stats.total_events == 1).await;
    assert_eq!(stats.status_2xx, 1);
    assert!(
        stats.total_upstream_pool_hits + stats.total_upstream_pool_misses > 0,
        "expected a real upstream peer observation, got {stats:?}"
    );
    Ok(())
}
