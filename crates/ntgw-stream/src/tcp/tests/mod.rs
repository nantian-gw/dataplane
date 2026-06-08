use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use super::{handle_connection, read_preface, run_with_listener};
use anyhow::Result;
use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendRef, Listener, SelectedBackendRuntimeIds,
    SharedSnapshot, Snapshot, StreamMatch, StreamRoute, StreamRule,
};
use ntgw_observability::{
    AccessLogMode, AccessLogOptions, SharedTrafficStats, shutdown_access_log_writer,
};
use tokio::{sync::watch, time::Duration};

const TCP_PROXY_BUFFER_BYTES: usize = 16 * 1024;

mod limits;
mod preface;
mod proxy;
mod routing;

fn disabled_access_log() -> AccessLogOptions {
    AccessLogOptions {
        enabled: false,
        ..AccessLogOptions::default()
    }
}

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

fn test_listener(name: &str, port: u32, protocol: &str) -> Listener {
    Listener {
        name: name.to_string(),
        address: "127.0.0.1".to_string(),
        addresses: vec!["127.0.0.1".to_string()],
        port,
        protocol: protocol.to_string(),
        hostnames: Vec::new(),
        attached_routes: vec![format!("default/{}", route_name_for_listener(name))],
        tls: None,
        backend_tls: None,
        metadata: BTreeMap::new(),
    }
}

fn route_name_for_listener(listener_name: &str) -> &str {
    match listener_name {
        "default/gw/tls" => "tls-route",
        _ => "tcp-route",
    }
}

fn test_snapshot(
    listener: Listener,
    route_name: &str,
    route_kind: &str,
    matches: Vec<StreamMatch>,
    upstream_addr: std::net::SocketAddr,
) -> SharedSnapshot {
    let shared = Snapshot::shared();
    *shared.write() = Snapshot {
        listeners: vec![listener],
        stream_routes: vec![StreamRoute {
            name: route_name.to_string(),
            namespace: "default".to_string(),
            kind: route_kind.to_string(),
            parent_refs: Vec::new(),
            rules: vec![StreamRule {
                name: String::new(),
                matches,
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "upstream".to_string(),
                    port: upstream_addr.port() as u32,
                    ..BackendRef::default()
                }],
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: format!("upstream:{}", upstream_addr.port()),
            namespace: "default".to_string(),
            protocol: "TCP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: upstream_addr.ip().to_string(),
                port: upstream_addr.port() as u32,
                healthy: true,
            }],
            wasm_plugin: None,
        }],
        ..Snapshot::default()
    };
    shared
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

fn build_client_hello(host: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&[0x03, 0x03]);
    body.extend_from_slice(&[0; 32]);
    body.push(0);
    body.extend_from_slice(&[0x00, 0x02, 0x00, 0x2f]);
    body.extend_from_slice(&[0x01, 0x00]);

    let mut server_name = Vec::new();
    server_name.extend_from_slice(&(host.len() as u16 + 3).to_be_bytes());
    server_name.push(0);
    server_name.extend_from_slice(&(host.len() as u16).to_be_bytes());
    server_name.extend_from_slice(host.as_bytes());

    let mut extensions = Vec::new();
    extensions.extend_from_slice(&[0x00, 0x00]);
    extensions.extend_from_slice(&(server_name.len() as u16).to_be_bytes());
    extensions.extend_from_slice(&server_name);

    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend_from_slice(&extensions);

    let mut handshake = vec![
        0x01,
        ((body.len() >> 16) & 0xff) as u8,
        ((body.len() >> 8) & 0xff) as u8,
        (body.len() & 0xff) as u8,
    ];
    handshake.extend_from_slice(&body);

    let mut record = Vec::new();
    record.extend_from_slice(&[0x16, 0x03, 0x01]);
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend_from_slice(&handshake);
    record
}
