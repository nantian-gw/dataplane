use ntgw_ir::Snapshot;
use ntgw_proto::gateway::control::v1 as proto;

#[test]
fn decodes_grpc_regex_match_type_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        grpc_routes: vec![proto::GrpcRoute {
            name: "grpc-route".to_string().into(),
            namespace: "default".to_string().into(),
            rules: vec![proto::GrpcRule {
                name: String::new(),
                matches: vec![proto::GrpcMatch {
                    service: "helloworld\\..+".to_string(),
                    method: "Say(H|G).*".to_string(),
                    match_type: "RegularExpression".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    });

    let matcher = &snapshot.grpc_routes[0].rules[0].matches[0];
    assert_eq!(matcher.service, "helloworld\\..+");
    assert_eq!(matcher.method, "Say(H|G).*");
    assert_eq!(matcher.match_type, "RegularExpression");
}
