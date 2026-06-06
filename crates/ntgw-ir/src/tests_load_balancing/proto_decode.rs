#[test]
fn decodes_backend_load_balancing_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        backends: vec![proto::BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            load_balancing: Some(proto::LoadBalancingPolicy {
                r#type: proto::LoadBalancingPolicyType::ConsistentHash as i32,
                consistent_hash: Some(proto::ConsistentHashPolicy {
                    key_type: proto::ConsistentHashKeyType::Header as i32,
                    header_name: "x-user-id".to_string(),
                }),
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
