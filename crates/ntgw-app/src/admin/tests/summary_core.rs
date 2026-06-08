use super::*;

mod listener_signals;
mod overload;
mod session_persistence;
mod surface_status;
mod transport_resources;

fn build_runtime_rejection_summary_value() -> serde_json::Value {
    let snapshot = Snapshot {
        id: "v1".to_string(),
        listeners: vec![
            Listener {
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                protocol: "LISTENER_PROTOCOL_TCP".to_string(),
                ..Listener::default()
            },
        ],
        http_routes: vec![Default::default()],
        grpc_routes: vec![GrpcRoute::default()],
        stream_routes: vec![Default::default()],
        backends: vec![Default::default()],
        secrets: vec![Default::default()],
        ..Snapshot::default()
    };
    let shared = Snapshot::shared();
    *shared.write() = snapshot;
    let xds = ClientStats::shared();
    xds.observe_connect_failure_with_error("dial tcp 127.0.0.1:18080: connection refused");
    xds.observe_stream_failure_with_error(
        "status: Unknown, message: \"h2 protocol error: error reading a body from connection\"",
    );
    xds.observe_snapshot_applied("v1");
    xds.observe_snapshot_nacked("v2", "listener reload failed");
    xds.observe_snapshot_skipped();
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_failure("v1", "web", "bind conflict");
    runtime.observe_http_tls_asset_reuses(2);
    runtime.observe_stream_listener_reload_failure("v1", "passthrough", "tcp bind conflict");

    let mut config = test_admin_runtime_config();
    config.http3_configured = true;
    let state = build_state_with_parts(config, shared, runtime, xds);

    build_summary_value(&state)
}

struct NamedListenerRuntimeIds {
    all: Vec<String>,
    web: String,
    passthrough: String,
}

fn build_named_listener_pending_summary_value() -> (serde_json::Value, NamedListenerRuntimeIds) {
    let mut snapshot = Snapshot {
        id: "v1".to_string(),
        listeners: vec![
            Listener {
                name: "web".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "passthrough".to_string(),
                protocol: "LISTENER_PROTOCOL_TCP".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();
    let expected = NamedListenerRuntimeIds {
        all: vec![
            snapshot
                .listener_runtime_id("web")
                .expect("web listener runtime id")
                .to_string(),
            snapshot
                .listener_runtime_id("passthrough")
                .expect("passthrough listener runtime id")
                .to_string(),
        ],
        web: snapshot
            .listener_runtime_id("web")
            .expect("web listener runtime id")
            .to_string(),
        passthrough: snapshot
            .listener_runtime_id("passthrough")
            .expect("passthrough listener runtime id")
            .to_string(),
    };
    let shared = Snapshot::shared();
    *shared.write() = snapshot;
    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        RuntimeStats::shared(),
        ClientStats::shared(),
    );

    (build_summary_value(&state), expected)
}
