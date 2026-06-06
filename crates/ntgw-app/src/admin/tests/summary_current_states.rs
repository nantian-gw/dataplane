use super::*;

include!("summary_current_states/current.rs");
include!("summary_current_states/convergence.rs");
include!("summary_current_states/failure_recovery.rs");
include!("summary_current_states/attention.rs");
include!("summary_current_states/serving.rs");

#[test]
fn summary_view_counts_listener_current_states() {
    let snapshot = Snapshot {
        id: "v2".to_string(),
        listeners: vec![
            Listener {
                name: "retained".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "accepted".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "stale".to_string(),
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                ..Listener::default()
            },
            Listener {
                name: "stream-pending".to_string(),
                protocol: "LISTENER_PROTOCOL_TCP".to_string(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    let shared = Snapshot::shared();
    *shared.write() = snapshot;
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_result("v1", &["retained".to_string()], &[], &[]);
    runtime.observe_http_listener_reload_result(
        "v2",
        &["accepted".to_string()],
        &["retained".to_string()],
        &[],
    );
    runtime.observe_http_listener_reload_result("v1", &["stale".to_string()], &[], &[]);

    let state = build_state_with_parts(
        test_admin_runtime_config(),
        shared,
        runtime,
        ClientStats::shared(),
    );

    let value = build_summary_value(&state);
    assert_current_listener_states(&value);
    assert_current_listener_convergence(&value);
    assert_current_listener_failure_recovery(&value);
    assert_current_listener_attention(&value);
    assert_current_listener_serving_and_warnings(&value);
}
