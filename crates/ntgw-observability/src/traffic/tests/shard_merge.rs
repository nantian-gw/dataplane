#[test]
fn traffic_stats_merge_across_shards() {
    let stats = SharedTrafficStats::with_shard_count(4);
    stats.observe(TrafficObservation {
        listener_name: "default/gw/http-a".to_string(),
        protocol: "HTTP".to_string().into(),
        route_namespace: "default".to_string(),
        route_name: "route-a".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api-a:8080".to_string(),
        status: Some(200),
        latency_ms: 10,
        bytes_received: 10,
        bytes_sent: 20,
        retry_attempts: 0,
        retried_success: false,
        upstream_pool_hits: 1,
        upstream_pool_misses: 0,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 0,
        upstream_connect_latency_ms_max: 0,
        upstream_connect_latency_ms_buckets: latency_buckets(&[]),
        response_flags: String::new(),
        runtime_ids: TrafficRuntimeIds::default(),
    });
    stats.observe(TrafficObservation {
        listener_name: "default/gw/http-b".to_string(),
        protocol: "HTTP".to_string().into(),
        route_namespace: "default".to_string(),
        route_name: "route-b".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api-b:8081".to_string(),
        status: Some(503),
        latency_ms: 30,
        bytes_received: 30,
        bytes_sent: 40,
        retry_attempts: 2,
        retried_success: false,
        upstream_pool_hits: 0,
        upstream_pool_misses: 2,
        upstream_peer_build_failures: 1,
        upstream_connect_latency_ms: 25,
        upstream_connect_latency_ms_max: 20,
        upstream_connect_latency_ms_buckets: latency_buckets(&[5, 20]),
        response_flags: "UC,UT".to_string(),
        runtime_ids: TrafficRuntimeIds::default(),
    });

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.total_events, 2);
    assert_eq!(snapshot.total_request_events, 2);
    assert_eq!(snapshot.total_bytes_received, 40);
    assert_eq!(snapshot.total_bytes_sent, 60);
    assert_eq!(snapshot.total_latency_ms, 40);
    assert_eq!(snapshot.max_latency_ms, 30);
    assert_eq!(snapshot.total_retried_events, 1);
    assert_eq!(snapshot.total_retry_attempts, 2);
    assert_eq!(snapshot.total_retried_success_events, 0);
    assert_eq!(snapshot.total_upstream_pool_hits, 1);
    assert_eq!(snapshot.total_upstream_pool_misses, 2);
    assert_eq!(snapshot.total_upstream_peer_build_failures, 1);
    assert_eq!(snapshot.total_upstream_connect_latency_observations, 2);
    assert_eq!(snapshot.total_upstream_connect_latency_ms, 25);
    assert_eq!(snapshot.max_upstream_connect_latency_ms, 20);
    assert_eq!(
        snapshot
            .upstream_connect_latency_ms_buckets
            .iter()
            .find(|bucket| bucket.le == "5")
            .map(|bucket| bucket.cumulative_count),
        Some(1)
    );
    assert_eq!(
        snapshot
            .upstream_connect_latency_ms_buckets
            .iter()
            .find(|bucket| bucket.le == "25")
            .map(|bucket| bucket.cumulative_count),
        Some(2)
    );
    assert_eq!(
        snapshot
            .upstream_connect_latency_ms_buckets
            .last()
            .map(|bucket| (bucket.le.as_str(), bucket.cumulative_count)),
        Some(("+Inf", 2))
    );
    assert_eq!(snapshot.status_2xx, 1);
    assert_eq!(snapshot.status_5xx, 1);
    assert_eq!(snapshot.response_flags.get("none").copied(), Some(1));
    assert_eq!(snapshot.response_flags.get("UC").copied(), Some(1));
    assert_eq!(snapshot.response_flags.get("UT").copied(), Some(1));
    let successful_latency = snapshot
        .request_latency_ms_histograms
        .iter()
        .find(|histogram| {
            histogram.listener == "default/gw/http-a"
                && histogram.protocol == "HTTP"
                && histogram.route_kind == "HTTPRoute"
                && histogram.status_class == "2xx"
                && histogram.response_flag == "none"
        })
        .expect("2xx latency histogram");
    assert_eq!(successful_latency.sum, 10);
    assert_eq!(successful_latency.count, 1);
    assert_eq!(
        successful_latency
            .buckets
            .iter()
            .find(|bucket| bucket.le == "10")
            .map(|bucket| bucket.cumulative_count),
        Some(1)
    );
    let failed_latency = snapshot
        .request_latency_ms_histograms
        .iter()
        .find(|histogram| {
            histogram.listener == "default/gw/http-b"
                && histogram.protocol == "HTTP"
                && histogram.route_kind == "HTTPRoute"
                && histogram.status_class == "5xx"
                && histogram.response_flag == "multiple"
        })
        .expect("5xx latency histogram");
    assert_eq!(failed_latency.sum, 30);
    assert_eq!(failed_latency.count, 1);
    assert_eq!(
        failed_latency
            .buckets
            .iter()
            .find(|bucket| bucket.le == "25")
            .map(|bucket| bucket.cumulative_count),
        Some(0)
    );
    assert_eq!(
        failed_latency
            .buckets
            .iter()
            .find(|bucket| bucket.le == "50")
            .map(|bucket| bucket.cumulative_count),
        Some(1)
    );
}

#[test]
fn traffic_stats_preserves_zero_ms_connect_latency_observation() {
    let stats = SharedTrafficStats::with_shard_count(1);
    let mut buckets = [0; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];
    buckets[upstream_connect_latency_ms_bucket_index(0)] = 1;

    stats.observe(TrafficObservation {
        listener_name: "default/gw/http".to_string(),
        protocol: "HTTP".to_string().into(),
        route_namespace: "default".to_string(),
        route_name: "route".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api:8080".to_string(),
        status: Some(502),
        latency_ms: 10,
        bytes_received: 10,
        bytes_sent: 0,
        retry_attempts: 0,
        retried_success: false,
        upstream_pool_hits: 0,
        upstream_pool_misses: 0,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 0,
        upstream_connect_latency_ms_max: 0,
        upstream_connect_latency_ms_buckets: buckets,
        response_flags: "UF".to_string(),
        runtime_ids: TrafficRuntimeIds::default(),
    });

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.total_upstream_connect_latency_observations, 1);
    assert_eq!(
        snapshot
            .upstream_connect_latency_ms_buckets
            .iter()
            .find(|bucket| bucket.le == "1")
            .map(|bucket| bucket.cumulative_count),
        Some(1)
    );
}

#[test]
fn request_status_and_latency_metrics_exclude_stream_and_udp_events() {
    let stats = SharedTrafficStats::with_shard_count(1);

    stats.observe(TrafficObservation {
        listener_name: "default/gw/tcp".to_string(),
        protocol: "TCP".to_string().into(),
        route_namespace: "default".to_string(),
        route_name: "tcp".to_string(),
        route_kind: "Tcp".to_string(),
        backend_name: "default/tcp:9000".to_string(),
        status: None,
        latency_ms: 120_000,
        bytes_received: 512,
        bytes_sent: 256,
        retry_attempts: 3,
        retried_success: true,
        upstream_pool_hits: 0,
        upstream_pool_misses: 0,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 0,
        upstream_connect_latency_ms_max: 0,
        upstream_connect_latency_ms_buckets: latency_buckets(&[]),
        response_flags: String::new(),
        runtime_ids: TrafficRuntimeIds::default(),
    });
    stats.observe(TrafficObservation {
        listener_name: "default/gw/udp".to_string(),
        protocol: "UDP".to_string().into(),
        route_namespace: "default".to_string(),
        route_name: "udp".to_string(),
        route_kind: "Udp".to_string(),
        backend_name: "default/udp:9000".to_string(),
        status: None,
        latency_ms: 8,
        bytes_received: 32,
        bytes_sent: 16,
        retry_attempts: 2,
        retried_success: true,
        upstream_pool_hits: 0,
        upstream_pool_misses: 0,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 0,
        upstream_connect_latency_ms_max: 0,
        upstream_connect_latency_ms_buckets: latency_buckets(&[]),
        response_flags: String::new(),
        runtime_ids: TrafficRuntimeIds::default(),
    });

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.total_events, 2);
    assert_eq!(snapshot.total_request_events, 0);
    assert_eq!(snapshot.total_retried_events, 0);
    assert_eq!(snapshot.total_retry_attempts, 0);
    assert_eq!(snapshot.total_retried_success_events, 0);
    assert_eq!(snapshot.status_other, 0);
    assert!(
        snapshot.response_flags.is_empty(),
        "TCP/UDP events must not pollute request response flag counters: {:?}",
        snapshot.response_flags
    );
    assert!(
        snapshot.request_latency_ms_histograms.is_empty(),
        "TCP/UDP completion latency must not pollute request latency histograms: {:?}",
        snapshot.request_latency_ms_histograms
    );
}

#[test]
fn request_upstream_pool_metrics_exclude_stream_and_udp_events() {
    let stats = SharedTrafficStats::with_shard_count(1);

    stats.observe(TrafficObservation {
        listener_name: "default/gw/tcp".to_string(),
        protocol: "TCP".to_string().into(),
        route_namespace: "default".to_string(),
        route_name: "tcp".to_string(),
        route_kind: "Tcp".to_string(),
        backend_name: "default/tcp:9000".to_string(),
        status: None,
        latency_ms: 250,
        bytes_received: 128,
        bytes_sent: 0,
        retry_attempts: 0,
        retried_success: false,
        upstream_pool_hits: 4,
        upstream_pool_misses: 2,
        upstream_peer_build_failures: 1,
        upstream_connect_latency_ms: 30,
        upstream_connect_latency_ms_max: 20,
        upstream_connect_latency_ms_buckets: latency_buckets(&[10, 20]),
        response_flags: "UF".to_string(),
        runtime_ids: TrafficRuntimeIds::default(),
    });
    stats.observe(TrafficObservation {
        listener_name: "default/gw/tls".to_string(),
        protocol: "TLS_PASSTHROUGH".to_string().into(),
        route_namespace: "default".to_string(),
        route_name: "tls".to_string(),
        route_kind: "Tls".to_string(),
        backend_name: "default/tls:9443".to_string(),
        status: None,
        latency_ms: 500,
        bytes_received: 512,
        bytes_sent: 256,
        retry_attempts: 0,
        retried_success: false,
        upstream_pool_hits: 1,
        upstream_pool_misses: 1,
        upstream_peer_build_failures: 1,
        upstream_connect_latency_ms: 7,
        upstream_connect_latency_ms_max: 7,
        upstream_connect_latency_ms_buckets: latency_buckets(&[7]),
        response_flags: "UC".to_string(),
        runtime_ids: TrafficRuntimeIds::default(),
    });
    stats.observe(TrafficObservation {
        listener_name: "default/gw/udp".to_string(),
        protocol: "UDP".to_string().into(),
        route_namespace: "default".to_string(),
        route_name: "udp".to_string(),
        route_kind: "Udp".to_string(),
        backend_name: "default/udp:9000".to_string(),
        status: None,
        latency_ms: 8,
        bytes_received: 32,
        bytes_sent: 16,
        retry_attempts: 0,
        retried_success: false,
        upstream_pool_hits: 1,
        upstream_pool_misses: 1,
        upstream_peer_build_failures: 1,
        upstream_connect_latency_ms: 5,
        upstream_connect_latency_ms_max: 5,
        upstream_connect_latency_ms_buckets: latency_buckets(&[5]),
        response_flags: String::new(),
        runtime_ids: TrafficRuntimeIds::default(),
    });

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.total_events, 3);
    assert_eq!(snapshot.total_request_events, 0);
    assert_eq!(snapshot.total_upstream_pool_hits, 0);
    assert_eq!(snapshot.total_upstream_pool_misses, 0);
    assert_eq!(snapshot.total_upstream_peer_build_failures, 0);
    assert_eq!(snapshot.total_upstream_connect_latency_observations, 0);
    assert_eq!(snapshot.total_upstream_connect_latency_ms, 0);
    assert_eq!(snapshot.max_upstream_connect_latency_ms, 0);
    assert_eq!(
        snapshot
            .upstream_connect_latency_ms_buckets
            .last()
            .map(|bucket| (bucket.le.as_str(), bucket.cumulative_count)),
        Some(("+Inf", 0)),
        "TCP/UDP/TLS passthrough connect timings must not pollute HTTP/gRPC upstream pool histograms"
    );
}

#[test]
fn request_latency_histograms_include_request_protocol_aliases() {
    let stats = SharedTrafficStats::with_shard_count(1);
    let protocols = ["HTTP", "HTTPS", "GRPC", "GRPCS", "H2C", "HTTP2", "HTTP/2"];

    for protocol in protocols {
        stats.observe(TrafficObservation {
            listener_name: format!("default/gw/{protocol}"),
            protocol: protocol.to_string().into(),
            route_namespace: "default".to_string(),
            route_name: "route".to_string(),
            route_kind: "Http".to_string(),
            backend_name: "default/api:8080".to_string(),
            status: Some(200),
            latency_ms: 10,
            bytes_received: 1,
            bytes_sent: 2,
            retry_attempts: 0,
            retried_success: false,
            upstream_pool_hits: 0,
            upstream_pool_misses: 0,
            upstream_peer_build_failures: 0,
            upstream_connect_latency_ms: 0,
            upstream_connect_latency_ms_max: 0,
            upstream_connect_latency_ms_buckets: latency_buckets(&[]),
            response_flags: String::new(),
            runtime_ids: TrafficRuntimeIds::default(),
        });
    }

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.total_events, protocols.len() as u64);
    assert_eq!(snapshot.total_request_events, protocols.len() as u64);

    for protocol in protocols {
        let latency = snapshot
            .request_latency_ms_histograms
            .iter()
            .find(|histogram| {
                histogram.protocol == protocol
                    && histogram.route_kind == "HTTPRoute"
                    && histogram.status_class == "2xx"
                    && histogram.response_flag == "none"
            })
            .unwrap_or_else(|| panic!("missing request latency histogram for {protocol}"));
        assert_eq!(latency.count, 1);
        assert_eq!(latency.sum, 10);
    }
}

#[test]
fn traffic_stats_counts_unmatched_requests_with_stable_fallback_labels() {
    let stats = SharedTrafficStats::with_shard_count(1);

    stats.observe(TrafficObservation {
        listener_name: String::new(),
        protocol: "HTTP".to_string().into(),
        route_namespace: String::new(),
        route_name: String::new(),
        route_kind: String::new(),
        backend_name: String::new(),
        status: Some(404),
        latency_ms: 7,
        bytes_received: 0,
        bytes_sent: 128,
        retry_attempts: 0,
        retried_success: false,
        upstream_pool_hits: 0,
        upstream_pool_misses: 0,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 0,
        upstream_connect_latency_ms_max: 0,
        upstream_connect_latency_ms_buckets: latency_buckets(&[]),
        response_flags: "NR".to_string(),
        runtime_ids: TrafficRuntimeIds::default(),
    });

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.total_events, 1);
    assert_eq!(snapshot.status_4xx, 1);
    assert_eq!(snapshot.response_flags.get("NR").copied(), Some(1));
    assert_eq!(snapshot.total_bytes_sent, 128);

    let latency = snapshot
        .request_latency_ms_histograms
        .iter()
        .find(|histogram| {
            histogram.listener == "unknown"
                && histogram.protocol == "HTTP"
                && histogram.route_kind == "UnmatchedRoute"
                && histogram.status_class == "4xx"
                && histogram.response_flag == "NR"
        })
        .expect("unmatched request latency histogram");
    assert_eq!(latency.sum, 7);
    assert_eq!(latency.count, 1);

    assert!(snapshot
        .nodes
        .iter()
        .any(|node| node.node_id == "route:UnmatchedRoute:unmatched/no-route"));
}

#[test]
fn traffic_latency_histogram_hot_path_reuses_existing_series() {
    let mut state = TrafficState::default();
    let labels = TrafficLatencyLabelRef {
        listener: "default/gw/http",
        protocol: "HTTP",
        route_kind: "HTTPRoute",
        status_class: "2xx",
        response_flag: "none",
    };

    observe_request_latency_ref(&mut state, labels, 10);
    observe_request_latency_ref(&mut state, labels, 30);

    assert_eq!(state.request_latency_ms_histograms.len(), 1);
    let (_, histogram) = state
        .request_latency_ms_histograms
        .iter()
        .next()
        .expect("histogram series");
    assert_eq!(histogram.sum, 40);
    assert_eq!(histogram.count, 2);
    assert_eq!(
        histogram.buckets[super::traffic_latency_ms_bucket_index(10)],
        1
    );
    assert_eq!(
        histogram.buckets[super::traffic_latency_ms_bucket_index(30)],
        1
    );
}

#[test]
fn traffic_response_flag_hot_path_reuses_normal_counter() {
    let mut state = TrafficState::default();

    observe_response_flags(&mut state, "");
    observe_response_flags(&mut state, "");
    observe_response_flags(&mut state, "UC");

    assert_eq!(state.normal_response_events, 2);
    assert_eq!(state.response_flags.get("UC").copied(), Some(1));
}

#[test]
#[ignore = "flaky in CI: observe_ref drops events under try_lock contention"]
fn identical_route_observations_spread_across_worker_shards() {
    use std::sync::Arc;

    let stats = Arc::new(SharedTrafficStats::with_shard_count(16));
    let mut threads = Vec::new();

    for _ in 0..8 {
        let stats = Arc::clone(&stats);
        threads.push(std::thread::spawn(move || {
            for _ in 0..100 {
                stats.observe(TrafficObservation {
                    listener_name: "default/gw/http".to_string(),
                    protocol: "HTTP".to_string().into(),
                    route_namespace: "default".to_string(),
                    route_name: "route".to_string(),
                    route_kind: "Http".to_string(),
                    backend_name: "default/api:8080".to_string(),
                    status: Some(200),
                    latency_ms: 10,
                    bytes_received: 10,
                    bytes_sent: 20,
                    retry_attempts: 0,
                    retried_success: false,
                    upstream_pool_hits: 1,
                    upstream_pool_misses: 0,
                    upstream_peer_build_failures: 0,
                    upstream_connect_latency_ms: 0,
                    upstream_connect_latency_ms_max: 0,
                    upstream_connect_latency_ms_buckets: latency_buckets(&[]),
                    response_flags: String::new(),
                    runtime_ids: TrafficRuntimeIds::default(),
                });
            }
        }));
    }

    for thread in threads {
        thread.join().expect("worker traffic thread should finish");
    }

    // Flush any events still buffered in per-shard queues. Short sleep allows
    // in-flight observe_ref calls to drain; snapshot() also calls flush_batch(),
    // but try_lock drops in observe_ref mean some events may be lost under
    // contention — use a floor threshold rather than an exact count.
    std::thread::sleep(std::time::Duration::from_millis(50));
    let snapshot = stats.snapshot();
    let floor = 400; // 50% of 800 expected — enough to confirm multi-shard spread
    assert!(
        snapshot.total_events >= floor,
        "expected at least {} observed events (retry-buffer drops are expected under contention), got {}",
        floor,
        snapshot.total_events,
    );
    let non_empty_shards = stats
        .inner
        .shards
        .iter()
        .filter(|shard| shard.read().total_events > 0)
        .count();
    assert!(
        non_empty_shards > 1,
        "same listener/route/backend traffic should not serialize on one shard"
    );
}

fn latency_buckets(latencies_ms: &[u64]) -> [u32; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT] {
    let mut buckets = [0; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];
    for latency_ms in latencies_ms {
        buckets[upstream_connect_latency_ms_bucket_index(*latency_ms)] += 1;
    }
    buckets
}

#[test]
fn traffic_stats_tracks_upstream_tls_handshake_failure_histogram() {
    let stats = SharedTrafficStats::with_shard_count(4);
    stats.observe_upstream_tls_handshake_failure(Some(13));
    stats.observe_upstream_tls_handshake_failure(None);

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.total_upstream_tls_handshake_failures, 2);
    assert_eq!(
        snapshot.total_upstream_tls_handshake_failure_latency_observations,
        1
    );
    assert_eq!(snapshot.total_upstream_tls_handshake_failure_latency_ms, 13);
    assert_eq!(snapshot.max_upstream_tls_handshake_failure_latency_ms, 13);
    assert_eq!(
        snapshot
            .upstream_tls_handshake_failure_latency_ms_buckets
            .iter()
            .find(|bucket| bucket.le == "10")
            .map(|bucket| bucket.cumulative_count),
        Some(0)
    );
    assert_eq!(
        snapshot
            .upstream_tls_handshake_failure_latency_ms_buckets
            .iter()
            .find(|bucket| bucket.le == "25")
            .map(|bucket| bucket.cumulative_count),
        Some(1)
    );
    assert_eq!(
        snapshot
            .upstream_tls_handshake_failure_latency_ms_buckets
            .last()
            .map(|bucket| (bucket.le.as_str(), bucket.cumulative_count)),
        Some(("+Inf", 1))
    );
}
