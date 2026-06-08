use super::*;

include!("surface_status/meta_health.rs");
include!("surface_status/snapshot_runtime.rs");

#[test]
fn summary_view_exposes_meta_health_snapshot_and_runtime_overviews() {
    let value = build_runtime_rejection_summary_value();

    assert_meta_health_and_warning_overviews(&value);
    assert_snapshot_and_runtime_overviews(&value);
    assert_eq!(value["snapshotVersion"], "v1");
}
