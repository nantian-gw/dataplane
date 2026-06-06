use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendRef, Listener, SelectedBackendRuntimeIds,
    SharedSnapshot, Snapshot, StreamMatch, StreamRoute, StreamRule,
};
use ntgw_observability::{
    shutdown_access_log_writer, AccessLogMode, AccessLogOptions, OverloadStats, SharedTrafficStats,
    UdpAdmissionController, UdpAdmissionOptions, UdpSessionStats,
};
use anyhow::Result;
use tokio::{
    net::UdpSocket,
    sync::{oneshot, watch},
    time::{timeout, Duration},
};

use super::{
    build_udp_session_task, proxy_datagram, run_with_socket, UdpSessionKey, UdpSessionRegistry,
};

include!("tests/support.rs");
include!("tests/proxy.rs");
include!("tests/budget.rs");
include!("tests/sessions.rs");
include!("tests/routing.rs");

fn json_access_log(path: &Path) -> AccessLogOptions {
    AccessLogOptions {
        path: path.display().to_string(),
        mode: AccessLogMode::Json,
        ..AccessLogOptions::default()
    }
}

fn temp_log_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("ntgw-stream-{prefix}-{unique}.log"))
}

async fn wait_for_log_contents(path: &Path, needle: &str) -> Result<String> {
    for _ in 0..100 {
        if let Ok(contents) = fs::read_to_string(path) {
            if contents.contains(needle) {
                return Ok(contents);
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    Err(anyhow::anyhow!(
        "timed out waiting for access log {} to contain {needle}",
        path.display()
    ))
}

fn assert_runtime_id_fields(contents: &str, expected: SelectedBackendRuntimeIds) {
    let fields = [
        ("listenerRuntimeId", expected.listener),
        ("routeRuntimeId", expected.route),
        ("ruleRuntimeId", expected.rule),
        ("backendRuntimeId", expected.backend),
        ("endpointRuntimeId", expected.endpoint),
    ];
    for (field, id) in fields {
        let id = id.expect("runtime id should be indexed");
        assert!(
            contents.contains(&format!("\"{field}\":\"{id}\"")),
            "access log should include {field}={id}: {contents}"
        );
    }
}

fn cleanup_access_log(path: &Path) {
    shutdown_access_log_writer(&path.display().to_string());
    let _ = fs::remove_file(path);
}

fn rebuild_runtime_indexes(snapshot: &SharedSnapshot) {
    snapshot.write().rebuild_runtime_indexes();
}

fn selected_runtime_ids(
    snapshot: &SharedSnapshot,
    listener_name: &str,
) -> SelectedBackendRuntimeIds {
    let current = snapshot.read();
    let selected = current
        .select_stream_backend(listener_name, None)
        .expect("stream backend should match");
    current.selected_backend_runtime_ids(&selected)
}
