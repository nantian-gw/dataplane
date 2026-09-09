#[test]
fn latency_bucket_indexes_match_public_bounds() {
    assert_bucket_indexes_match_bounds(
        &super::TRAFFIC_LATENCY_MS_BUCKET_BOUNDS,
        super::traffic_latency_ms_bucket_index,
    );
    assert_bucket_indexes_match_bounds(
        &super::UPSTREAM_CONNECT_LATENCY_MS_BUCKET_BOUNDS,
        super::upstream_connect_latency_ms_bucket_index,
    );
}

fn assert_bucket_indexes_match_bounds(bounds: &[u64], bucket_index: impl Fn(u64) -> usize) {
    for (index, bound) in bounds.iter().copied().enumerate() {
        assert_eq!(bucket_index(bound), index);
        if bound > 0 {
            assert_eq!(bucket_index(bound - 1), index);
        }
        assert_eq!(bucket_index(bound.saturating_add(1)), index + 1);
    }
    assert_eq!(bucket_index(u64::MAX), bounds.len());
}
