#[test]
fn decodes_backend_load_balancing_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        backends: vec![proto::BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            load_balancing: Some(proto::LoadBalancingPolicy {
                r#type: proto::LoadBalancingPolicyType::LoadBalancingConsistentHash as i32,
                consistent_hash: Some(proto::ConsistentHashPolicy {
                    key_type: proto::ConsistentHashKeyType::ConsistentHashHeader as i32,
                    header_name: "x-user-id".to_string(),
                }),
                slow_start: None,
            }),
            wasm_plugin: None,
            ..Default::default()
        }],
        ..Default::default()
    });

    let policy = snapshot
        .backend_policy("default/orders:8080")
        .and_then(|policy| policy.load_balancing.as_ref())
        .expect("backend load balancing");
    assert_eq!(policy.policy_type, "ConsistentHash");
    assert_eq!(
        policy
            .consistent_hash
            .as_ref()
            .expect("consistent hash")
            .key_type,
        "Header"
    );
    assert_eq!(
        policy
            .consistent_hash
            .as_ref()
            .expect("consistent hash")
            .header_name,
        "x-user-id"
    );
}

#[test]
fn decodes_backend_health_check_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        backends: vec![proto::BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            health_check: Some(proto::HealthCheckConfig {
                r#type: "HTTP".to_string(),
                path: "/healthz".to_string(),
                expected_status: 200,
                healthy_threshold: 2,
                unhealthy_threshold: 2,
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    });

    let hc = snapshot
        .backend_policy("default/orders:8080")
        .and_then(|policy| policy.health_check.as_ref())
        .expect("health check");
    assert_eq!(hc.r#type, "HTTP");
    assert_eq!(hc.path, "/healthz");
    assert_eq!(hc.expected_status, 200);
    assert_eq!(hc.healthy_threshold, 2);
}

#[test]
fn decodes_outlier_detection_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        backends: vec![proto::BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            outlier_detection: Some(proto::OutlierDetectionConfig {
                consecutive_5xx: 5,
                max_ejection_percent: 50,
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    });

    let od = snapshot
        .backend_policy("default/orders:8080")
        .and_then(|policy| policy.outlier_detection.as_ref())
        .expect("outlier detection");
    assert_eq!(od.consecutive_5xx, 5);
    assert_eq!(od.max_ejection_percent, 50);
}

#[test]
fn decodes_least_request_and_slow_start_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        backends: vec![proto::BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            load_balancing: Some(proto::LoadBalancingPolicy {
                r#type: proto::LoadBalancingPolicyType::LoadBalancingLeastRequest as i32,
                slow_start: Some(proto::SlowStartConfig {
                    window: Some(prost_types::Duration {
                        seconds: 30,
                        nanos: 0,
                    }),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    });

    let lb = snapshot
        .backend_policy("default/orders:8080")
        .and_then(|policy| policy.load_balancing.as_ref())
        .expect("load balancing");
    assert_eq!(lb.policy_type, "LeastRequest");
    assert_eq!(
        lb.slow_start.as_ref().and_then(|s| s.window),
        Some(std::time::Duration::from_secs(30))
    );
}
