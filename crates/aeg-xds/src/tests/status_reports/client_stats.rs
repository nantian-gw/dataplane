#[test]
fn client_stats_retain_last_transport_errors() {
    let stats = super::super::ClientStats::shared();

    stats.observe_connect_failure_with_error("connection refused");
    stats.observe_stream_failure_with_error("h2 protocol error");

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.connect_failures, 1);
    assert_eq!(snapshot.stream_failures, 1);
    assert_eq!(snapshot.last_connect_error, "connection refused");
    assert_eq!(snapshot.last_stream_error, "h2 protocol error");
    assert!(snapshot.last_connect_failure_unix_seconds > 0);
    assert!(snapshot.last_stream_failure_unix_seconds > 0);
}

#[test]
fn client_stats_track_stream_connectivity() {
    let stats = super::super::ClientStats::shared();

    stats.observe_stream_connected();

    let snapshot = stats.snapshot();
    assert!(snapshot.stream_connected);
    assert!(snapshot.last_control_plane_contact_unix_seconds > 0);
}

#[test]
fn client_stats_clear_connectivity_on_stream_failure() {
    let stats = super::super::ClientStats::shared();
    stats.observe_stream_connected();

    stats.observe_stream_failure_with_error("rpc closed");

    let snapshot = stats.snapshot();
    assert!(!snapshot.stream_connected);
    assert_eq!(snapshot.last_stream_error, "rpc closed");
}

#[test]
fn client_stats_track_apply_stage_duration_histograms() {
    let stats = super::super::ClientStats::shared();

    stats.observe_apply_stage_duration("decode", 7);
    stats.observe_apply_stage_duration("decode", 44);
    stats.observe_apply_stage_duration("snapshot_swap", 0);

    let snapshot = stats.snapshot();
    let decode = snapshot
        .apply_stage_ms_histograms
        .iter()
        .find(|histogram| histogram.stage == "decode")
        .expect("decode stage histogram");
    assert_eq!(decode.sum, 51);
    assert_eq!(decode.count, 2);
    assert_eq!(
        decode
            .buckets
            .iter()
            .find(|bucket| bucket.le == "10")
            .map(|bucket| bucket.cumulative_count),
        Some(1)
    );
    assert_eq!(
        decode
            .buckets
            .iter()
            .find(|bucket| bucket.le == "50")
            .map(|bucket| bucket.cumulative_count),
        Some(2)
    );

    let snapshot_swap = snapshot
        .apply_stage_ms_histograms
        .iter()
        .find(|histogram| histogram.stage == "snapshot_swap")
        .expect("snapshot_swap stage histogram");
    assert_eq!(snapshot_swap.sum, 0);
    assert_eq!(snapshot_swap.count, 1);
}

#[test]
fn client_stats_can_be_used_as_apply_stage_recorder() {
    let stats = super::super::ClientStats::shared();
    let recorder: aeg_observability::SharedApplyStageRecorder = stats.clone();

    recorder.observe_apply_stage_duration("listener_plan", 12);

    let snapshot = stats.snapshot();
    let listener_plan = snapshot
        .apply_stage_ms_histograms
        .iter()
        .find(|histogram| histogram.stage == "listener_plan")
        .expect("listener_plan stage histogram");
    assert_eq!(listener_plan.sum, 12);
    assert_eq!(listener_plan.count, 1);
}

#[test]
fn xds_apply_path_records_core_stage_durations() {
    let source = include_str!("../../lib.rs");
    for stage in [
        "decode",
        "inherit_runtime_state",
        "rebuild_indexes",
        "snapshot_swap",
        "listener_apply",
        "ack_wait",
    ] {
        assert!(
            source.contains(&format!("observe_apply_stage_elapsed(&stats, \"{stage}\"")),
            "xDS apply path should record {stage} stage duration"
        );
    }
}
