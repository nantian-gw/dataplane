use super::*;

include!("listener_signals/state.rs");
include!("listener_signals/convergence.rs");
include!("listener_signals/failure_recovery.rs");
include!("listener_signals/attention.rs");
include!("listener_signals/overviews.rs");

#[test]
fn summary_view_exposes_listener_state_and_signal_overviews() {
    let value = build_runtime_rejection_summary_value();

    assert_listener_state_summary(&value);
    assert_listener_convergence_summary(&value);
    assert_listener_failure_recovery_summary(&value);
    assert_listener_attention_summary(&value);
    assert_listener_overviews(&value);
}

#[test]
fn summary_listener_signal_fields_expose_runtime_ids() {
    let (value, expected) = build_named_listener_pending_summary_value();

    assert_eq!(
        value["listenerCurrentPendingNames"],
        serde_json::json!(["web", "passthrough"])
    );
    assert_eq!(
        value["listenerCurrentPendingRuntimeIds"],
        serde_json::json!(expected.all)
    );
    assert_eq!(
        value["listenerConvergenceBlockedRuntimeIds"],
        serde_json::json!(expected.all)
    );
    assert_eq!(
        value["listenerConvergenceBlockedHttpRuntimeIds"],
        serde_json::json!([expected.web])
    );
    assert_eq!(
        value["listenerConvergenceBlockedStreamRuntimeIds"],
        serde_json::json!([expected.passthrough])
    );
    assert_eq!(
        value["listenerAttentionRequiredRuntimeIds"],
        serde_json::json!(expected.all)
    );
    assert_eq!(
        value["listenerAttentionPendingRuntimeIds"],
        serde_json::json!(expected.all)
    );
    assert_eq!(
        value["listenerServingCurrentSnapshotRuntimeIds"],
        serde_json::json!([])
    );
}
