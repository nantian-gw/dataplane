use super::*;

include!("recovery_counts/counts.rs");
include!("recovery_counts/failure_recovery.rs");
include!("recovery_counts/convergence.rs");
include!("recovery_counts/attention.rs");
include!("recovery_counts/overviews.rs");

#[test]
fn summary_view_counts_listener_recovery_states() {
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![
            Listener {
                name: "recovered".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "failed".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "clean".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    let shared = Snapshot::shared();
    *shared.write() = snapshot;
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_failure("v1", "recovered", "bind conflict");
    runtime.observe_http_listener_reload_result("v2", &["recovered".to_string()], &[], &[]);
    runtime.observe_http_listener_reload_failure("v2", "failed", "address in use");
    runtime.observe_http_listener_reload_result("v2", &["clean".to_string()], &[], &[]);

    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        runtime,
        ClientStats::shared(),
    );

    let value = build_summary_value(&state);
    assert_listener_recovery_counts(&value);
    assert_listener_failure_recovery(&value);
    assert_listener_convergence(&value);
    assert_listener_attention(&value);
    assert_listener_overviews_and_warnings(&value);
}
