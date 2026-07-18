use ntgw_ir::Snapshot;
use ntgw_proto::gateway::control::v1 as proto;

#[test]
fn from_proto_retains_http_grpc_and_stream_route_labels() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "http-route".to_string(),
            namespace: "default".to_string(),
            labels: std::collections::HashMap::from([("team".to_string(), "edge".to_string())]),
            ..Default::default()
        }],
        grpc_routes: vec![proto::GrpcRoute {
            name: "grpc-route".to_string(),
            namespace: "default".to_string(),
            labels: std::collections::HashMap::from([("team".to_string(), "api".to_string())]),
            ..Default::default()
        }],
        stream_routes: vec![proto::StreamRoute {
            name: "stream-route".to_string(),
            namespace: "default".to_string(),
            labels: std::collections::HashMap::from([("team".to_string(), "tcp".to_string())]),
            ..Default::default()
        }],
        ..Default::default()
    });

    assert_eq!(
        snapshot.http_routes[0].labels.get("team"),
        Some(&"edge".to_string())
    );
    assert_eq!(
        snapshot.grpc_routes[0].labels.get("team"),
        Some(&"api".to_string())
    );
    assert_eq!(
        snapshot.stream_routes[0].labels.get("team"),
        Some(&"tcp".to_string())
    );
}
