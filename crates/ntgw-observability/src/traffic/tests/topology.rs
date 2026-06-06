#[test]
fn traffic_stats_snapshot_uses_topology_ids() {
    let stats = SharedTrafficStats::shared();
    stats.observe(TrafficObservation {
        listener_name: "default/gw/http".to_string(),
        protocol: "HTTP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "web".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api:8080".to_string(),
        status: Some(200),
        latency_ms: 15,
        bytes_received: 128,
        bytes_sent: 512,
        retry_attempts: 1,
        retried_success: true,
        upstream_pool_hits: 1,
        upstream_pool_misses: 1,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 7,
        upstream_connect_latency_ms_max: 7,
        upstream_connect_latency_ms_buckets: latency_buckets(&[7]),
        response_flags: "DC".to_string(),
        runtime_ids: TrafficRuntimeIds {
            listener: Some(0x111),
            route: Some(0x222),
            backend: Some(0x333),
        },
    });

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.total_events, 1);
    assert_eq!(snapshot.total_bytes_received, 128);
    assert_eq!(snapshot.total_bytes_sent, 512);
    assert_eq!(snapshot.total_latency_ms, 15);
    assert_eq!(snapshot.max_latency_ms, 15);
    assert_eq!(snapshot.total_retried_events, 1);
    assert_eq!(snapshot.total_retry_attempts, 1);
    assert_eq!(snapshot.total_retried_success_events, 1);
    assert_eq!(snapshot.total_upstream_pool_hits, 1);
    assert_eq!(snapshot.total_upstream_pool_misses, 1);
    assert_eq!(snapshot.total_upstream_connect_latency_observations, 1);
    assert_eq!(snapshot.total_upstream_connect_latency_ms, 7);
    assert_eq!(snapshot.max_upstream_connect_latency_ms, 7);
    assert_eq!(snapshot.status_2xx, 1);
    assert_eq!(snapshot.response_flags.get("DC").copied(), Some(1));
    let listener = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == "listener:default/gw/http")
        .expect("listener node");
    assert_eq!(listener.runtime_id, Some(0x111));
    let route = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == "route:HTTPRoute:default/web")
        .expect("route node");
    assert_eq!(route.runtime_id, Some(0x222));
    let backend = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == "backend:default/api:8080")
        .expect("backend node");
    assert_eq!(backend.runtime_id, Some(0x333));
    assert!(snapshot
        .edges
        .iter()
        .any(|edge| edge.edge_id == "edge:listener:default/gw/http:route:HTTPRoute:default/web"));
    assert!(snapshot
        .edges
        .iter()
        .any(|edge| edge.edge_id == "edge:route:HTTPRoute:default/web:backend:default/api:8080"));
}

#[test]
fn borrowed_traffic_observation_matches_owned_observation() {
    let owned_stats = SharedTrafficStats::with_shard_count(1);
    let borrowed_stats = SharedTrafficStats::with_shard_count(1);
    let buckets = latency_buckets(&[7]);
    let runtime_ids = TrafficRuntimeIds {
        listener: Some(0x111),
        route: Some(0x222),
        backend: Some(0x333),
    };

    owned_stats.observe(TrafficObservation {
        listener_name: "default/gw/http".to_string(),
        protocol: "HTTP".to_string(),
        route_namespace: "default".to_string(),
        route_name: "web".to_string(),
        route_kind: "Http".to_string(),
        backend_name: "default/api:8080".to_string(),
        status: Some(200),
        latency_ms: 15,
        bytes_received: 128,
        bytes_sent: 512,
        retry_attempts: 1,
        retried_success: true,
        upstream_pool_hits: 1,
        upstream_pool_misses: 1,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 7,
        upstream_connect_latency_ms_max: 7,
        upstream_connect_latency_ms_buckets: buckets,
        response_flags: "DC".to_string(),
        runtime_ids,
    });
    borrowed_stats.observe_ref(TrafficObservationRef {
        listener_name: "default/gw/http",
        protocol: "HTTP",
        route_namespace: "default",
        route_name: "web",
        route_kind: "Http",
        backend_name: "default/api:8080",
        status: Some(200),
        latency_ms: 15,
        bytes_received: 128,
        bytes_sent: 512,
        retry_attempts: 1,
        retried_success: true,
        upstream_pool_hits: 1,
        upstream_pool_misses: 1,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: 7,
        upstream_connect_latency_ms_max: 7,
        upstream_connect_latency_ms_buckets: &buckets,
        response_flags: "DC",
        runtime_ids,
    });

    let mut owned_snapshot = serde_json::to_value(owned_stats.snapshot()).expect("owned snapshot");
    let mut borrowed_snapshot =
        serde_json::to_value(borrowed_stats.snapshot()).expect("borrowed snapshot");
    strip_last_seen_timestamps(&mut owned_snapshot);
    strip_last_seen_timestamps(&mut borrowed_snapshot);

    assert_eq!(borrowed_snapshot["total_events"], owned_snapshot["total_events"]);
    assert_eq!(
        borrowed_snapshot["total_upstream_connect_latency_ms"],
        owned_snapshot["total_upstream_connect_latency_ms"]
    );
    assert_eq!(borrowed_snapshot["nodes"], owned_snapshot["nodes"]);
    assert_eq!(borrowed_snapshot["edges"], owned_snapshot["edges"]);
    assert_eq!(
        borrowed_snapshot["request_latency_ms_histograms"],
        owned_snapshot["request_latency_ms_histograms"]
    );
}

#[test]
fn precomputed_traffic_topology_drives_nodes_and_edges() {
    let stats = SharedTrafficStats::with_shard_count(1);
    let buckets = latency_buckets(&[7]);
    let topology = TrafficTopology::from_parts(
        "default/gw/http",
        "Http",
        "default",
        "web",
        "default/api:8080",
    );

    stats.observe_ref_with_topology(
        TrafficObservationRef {
            listener_name: "default/gw/http",
            protocol: "HTTP",
            route_namespace: "default",
            route_name: "web",
            route_kind: "Http",
            backend_name: "opaque-backend-name",
            status: Some(200),
            latency_ms: 15,
            bytes_received: 128,
            bytes_sent: 512,
            retry_attempts: 0,
            retried_success: false,
            upstream_pool_hits: 1,
            upstream_pool_misses: 0,
            upstream_peer_build_failures: 0,
            upstream_connect_latency_ms: 7,
            upstream_connect_latency_ms_max: 7,
            upstream_connect_latency_ms_buckets: &buckets,
            response_flags: "",
            runtime_ids: TrafficRuntimeIds {
                listener: Some(0x111),
                route: Some(0x222),
                backend: Some(0x333),
            },
        },
        Some(topology.as_ref()),
    );

    let snapshot = stats.snapshot();

    assert!(snapshot
        .nodes
        .iter()
        .any(|node| node.node_id == "backend:default/api:8080"
            && node.runtime_id == Some(0x333)));
    assert!(snapshot.edges.iter().any(|edge| edge.edge_id
        == "edge:route:HTTPRoute:default/web:backend:default/api:8080"));
}

#[test]
fn topology_helpers_use_borrowed_fast_paths_for_common_gateway_labels() {
    use std::borrow::Cow;

    let http_kind = super::topology::canonical_route_kind_ref("Http");
    let grpc_kind = super::topology::canonical_route_kind_ref("Grpc");
    let backend = super::topology::parse_backend_name_ref("default/api:8080")
        .expect("backend name should parse");

    assert!(matches!(http_kind, Cow::Borrowed("HTTPRoute")));
    assert!(matches!(grpc_kind, Cow::Borrowed("GRPCRoute")));
    assert_eq!(backend.namespace, "default");
    assert_eq!(backend.name, "api");
    assert_eq!(backend.port, 8080);
}

#[test]
fn traffic_topology_precomputes_stable_shard_key() {
    let first = TrafficTopology::from_parts(
        "default/gw/http",
        "Http",
        "default",
        "web",
        "default/api:8080",
    );
    let second = TrafficTopology::from_parts(
        "default/gw/http",
        "Http",
        "default",
        "web",
        "default/api:8080",
    );
    let different_backend = TrafficTopology::from_parts(
        "default/gw/http",
        "Http",
        "default",
        "web",
        "default/other:8080",
    );

    assert_eq!(first.as_ref().shard_key, second.as_ref().shard_key);
    assert_ne!(
        first.as_ref().shard_key,
        different_backend.as_ref().shard_key
    );
}

fn strip_last_seen_timestamps(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            object.remove("last_seen_unix_ms");
            for value in object.values_mut() {
                strip_last_seen_timestamps(value);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                strip_last_seen_timestamps(value);
            }
        }
        _ => {}
    }
}
