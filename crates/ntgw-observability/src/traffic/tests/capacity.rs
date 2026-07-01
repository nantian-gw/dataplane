#[test]
fn traffic_stats_snapshot_caps_topology_growth_per_shard() {
    let stats = SharedTrafficStats::with_shard_count(1);

    for index in 0..=DEFAULT_TRAFFIC_NODE_LIMIT_PER_SHARD {
        stats.observe(TrafficObservation {
            listener_name: format!("default/gw/http-{index}"),
            protocol: "HTTP".to_string().into(),
            route_namespace: "default".to_string(),
            route_name: "web".to_string(),
            route_kind: "Http".to_string(),
            backend_name: "default/api:8080".to_string(),
            status: Some(200),
            latency_ms: 5,
            bytes_received: 10,
            bytes_sent: 20,
            retry_attempts: 0,
            retried_success: false,
            upstream_pool_hits: 0,
            upstream_pool_misses: 0,
            upstream_peer_build_failures: 0,
            upstream_connect_latency_ms: 0,
            upstream_connect_latency_ms_max: 0,
            upstream_connect_latency_ms_buckets: [0; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT],
            response_flags: String::new(),
            runtime_ids: TrafficRuntimeIds::default(),
        });
    }

    let snapshot = stats.snapshot();
    assert_eq!(
        snapshot.total_events,
        (DEFAULT_TRAFFIC_NODE_LIMIT_PER_SHARD + 1) as u64
    );
    assert!(
        snapshot.nodes.len() <= DEFAULT_TRAFFIC_NODE_LIMIT_PER_SHARD,
        "expected node count <= {}, got {}",
        DEFAULT_TRAFFIC_NODE_LIMIT_PER_SHARD,
        snapshot.nodes.len()
    );
    assert!(
        snapshot.edges.len() <= DEFAULT_TRAFFIC_EDGE_LIMIT_PER_SHARD,
        "expected edge count <= {}, got {}",
        DEFAULT_TRAFFIC_EDGE_LIMIT_PER_SHARD,
        snapshot.edges.len()
    );
    assert!(snapshot
        .nodes
        .iter()
        .any(|node| node.node_id == "listener:default/gw/http-1024"));
    assert!(!snapshot
        .nodes
        .iter()
        .any(|node| node.node_id == "listener:default/gw/http-0"));
    assert!(snapshot.edges.iter().any(|edge| {
        edge.edge_id == "edge:listener:default/gw/http-1024:route:HTTPRoute:default/web"
    }));
    assert!(!snapshot.edges.iter().any(|edge| {
        edge.edge_id == "edge:listener:default/gw/http-0:route:HTTPRoute:default/web"
    }));
}
