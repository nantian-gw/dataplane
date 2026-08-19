#[test]
fn decodes_named_rules_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "http".to_string(),
            namespace: "default".to_string(),
            rules: vec![proto::HttpRule {
                name: "http-primary".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }],
        security_policy: None,
        grpc_routes: vec![proto::GrpcRoute {
            name: "grpc".to_string(),
            namespace: "default".to_string(),
            rules: vec![proto::GrpcRule {
                name: "grpc-primary".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }],
        stream_routes: vec![proto::StreamRoute {
            name: "tcp".to_string(),
            namespace: "default".to_string(),
            kind: proto::RouteKind::Tcp as i32,
            rules: vec![proto::StreamRule {
                name: "tcp-primary".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }],
        security_policy: None,
        ..Default::default()
    });

    let http_rule = format!("{:?}", snapshot.http_routes[0].rules[0]);
    let grpc_rule = format!("{:?}", snapshot.grpc_routes[0].rules[0]);
    let stream_rule = format!("{:?}", snapshot.stream_routes[0].rules[0]);

    assert!(
        http_rule.contains("http-primary"),
        "decoded HTTP rule did not preserve name: {http_rule}"
    );
    assert!(
        grpc_rule.contains("grpc-primary"),
        "decoded gRPC rule did not preserve name: {grpc_rule}"
    );
    assert!(
        stream_rule.contains("tcp-primary"),
        "decoded stream rule did not preserve name: {stream_rule}"
    );
}
