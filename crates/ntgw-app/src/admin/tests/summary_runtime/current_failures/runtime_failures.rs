#[test]
fn summary_current_failures_exposes_runtime_failure_arrays() {
    let value = multiple_current_failures_summary_value();

    assert_eq!(
        value["runtimeHttpCurrentFailures"],
        serde_json::json!([
            { "listener": "web", "message": "bind conflict" },
            { "listener": "admin", "message": "address in use" }
        ])
    );
    assert_eq!(
        value["runtimeTlsCurrentFailures"],
        serde_json::json!([
            { "listener": "passthrough", "message": "tcp bind conflict" },
            { "listener": "udp", "message": "udp bind conflict" }
        ])
    );
    assert_eq!(
        value["currentSnapshotRejectionMessage"],
        "HTTP runtime: web: bind conflict; admin: address in use; TLS runtime: passthrough: tcp bind conflict; udp: udp bind conflict"
    );
}
