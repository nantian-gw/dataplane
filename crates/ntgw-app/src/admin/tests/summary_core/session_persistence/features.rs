#[test]
fn summary_session_persistence_ephemeral_secret_exposes_feature_counts() {
    let value = ephemeral_session_persistence_summary_value();

    assert_eq!(value["sessionPersistenceConfigured"], false);
    assert_eq!(value["sessionPersistenceUsesEphemeralSecret"], true);
    assert_eq!(value["sessionPersistenceActive"], true);
    assert_eq!(value["sessionPersistenceRouteRuleCount"], 1);
    assert_eq!(value["sessionPersistenceBackendPolicyCount"], 1);
    assert_eq!(
        value["featureOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "http3": {
                "status": {
                    "configured": false,
                    "available": ntgw_http::http3_available(),
                    "enabled": false,
                },
                "configured": false,
                "available": ntgw_http::http3_available(),
                "enabled": false,
            },
            "sessionPersistence": {
                "status": {
                    "configured": false,
                    "usesEphemeralSecret": true,
                    "active": true,
                },
                "counts": {
                    "routeRuleCount": 1,
                    "backendPolicyCount": 1,
                },
                "configured": false,
                "usesEphemeralSecret": true,
                "active": true,
                "routeRuleCount": 1,
                "backendPolicyCount": 1,
            }
        })
    );
}
