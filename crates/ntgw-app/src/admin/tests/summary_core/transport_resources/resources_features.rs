fn assert_resource_and_feature_overviews(value: &serde_json::Value) {
    assert_eq!(value["routeCount"], 3);
    assert_eq!(
        value["resourceOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "counts": {
                "listeners": {
                    "total": 2,
                    "http": 1,
                    "https": 0,
                    "stream": 1
                },
                "routes": {
                    "total": 3,
                    "http": 1,
                    "grpc": 1,
                    "stream": 1
                },
                "backends": 1,
                "secrets": 1
            },
            "listeners": {
                "total": 2,
                "http": 1,
                "https": 0,
                "stream": 1
            },
            "routes": {
                "total": 3,
                "http": 1,
                "grpc": 1,
                "stream": 1
            },
            "backends": 1,
            "secrets": 1
        })
    );
    assert_eq!(value["http3Configured"], true);
    assert_eq!(value["http3Available"], ntgw_http::http3_available());
    assert_eq!(value["sessionPersistenceConfigured"], true);
    assert_eq!(value["sessionPersistenceActive"], false);
    assert_eq!(
        value["featureOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "http3": {
                "status": {
                    "configured": true,
                    "available": ntgw_http::http3_available(),
                    "enabled": ntgw_http::http3_available(),
                },
                "configured": true,
                "available": ntgw_http::http3_available(),
                "enabled": ntgw_http::http3_available(),
            },
            "sessionPersistence": {
                "status": {
                    "configured": true,
                    "usesEphemeralSecret": false,
                    "active": false,
                },
                "counts": {
                    "routeRuleCount": 0,
                    "backendPolicyCount": 0,
                },
                "configured": true,
                "usesEphemeralSecret": false,
                "active": false,
                "routeRuleCount": 0,
                "backendPolicyCount": 0,
            }
        })
    );
}
