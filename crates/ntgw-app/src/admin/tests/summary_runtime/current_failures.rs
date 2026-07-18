fn multiple_current_failures_summary_value() -> serde_json::Value {
    let snapshot = Snapshot {
        id: "v-multi".to_string(),
        listeners: vec![
            Listener {
                name: "web".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "passthrough".to_string(),
                protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    let shared = Snapshot::shared();
    shared.store(Arc::new(snapshot));
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_failures(
        "v-multi",
        &[
            RuntimeListenerFailure {
                listener: "web".to_string(),
                message: "bind conflict".to_string(),
            },
            RuntimeListenerFailure {
                listener: "admin".to_string(),
                message: "address in use".to_string(),
            },
        ],
    );
    runtime.observe_tls_listener_reload_failures(
        "v-multi",
        &[
            RuntimeListenerFailure {
                listener: "passthrough".to_string(),
                message: "tcp bind conflict".to_string(),
            },
            RuntimeListenerFailure {
                listener: "udp".to_string(),
                message: "udp bind conflict".to_string(),
            },
        ],
    );

    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        runtime,
        ClientStats::shared(),
    );

    build_summary_value(&state)
}

include!("current_failures/runtime_failures.rs");
include!("current_failures/listener_blocking.rs");
include!("current_failures/failure_recovery_overview.rs");
include!("current_failures/attention_overview.rs");
