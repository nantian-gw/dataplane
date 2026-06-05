use super::*;

#[test]
fn collect_listener_runtime_statuses_filters_by_runtime_plane_and_serving_state() {
    let snapshot = fixture_snapshot();
    let runtime = RuntimeStats::shared();
    runtime.observe_http_listener_reload_result("v-test", &["web".to_string()], &[], &[]);
    runtime.observe_stream_listener_reload_result("v1", &["passthrough".to_string()], &[], &[]);

    let statuses = collect_listener_runtime_statuses(
        &snapshot,
        &runtime.snapshot(),
        &ListenerListQuery {
            runtime_plane: Some("http".to_string()),
            serving_state: Some("current-accepted".to_string()),
            ..ListenerListQuery::default()
        },
    )
    .expect("listener statuses should be collected");

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].listener.name, "web");
    assert_eq!(statuses[0].runtime_plane, "http");
    assert_eq!(statuses[0].listener_serving_state, "current-accepted");
}
