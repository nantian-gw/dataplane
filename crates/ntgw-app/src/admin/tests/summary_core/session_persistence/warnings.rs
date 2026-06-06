#[test]
fn summary_session_persistence_ephemeral_secret_exposes_warning_overview() {
    let value = ephemeral_session_persistence_summary_value();

    assert_eq!(
        value["warningOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "status": {
                "count": 1,
                "hasWarnings": true,
                "primaryCategory": "session-persistence-ephemeral-secret",
                "primaryMessage": "session persistence is active but the dataplane is using an ephemeral secret; configure sessionPersistence.secretKey or secretKeyFile for restart-stable multi-replica deployments"
            },
            "count": 1,
            "hasWarnings": true,
            "categories": [
                "session-persistence-ephemeral-secret"
            ],
            "counts": {
                "sessionPersistenceEphemeralSecret": 1,
                "runtimeHttpReloadFailure": 0,
                "runtimeTlsReloadFailure": 0,
                "runtimeStreamReloadFailure": 0,
                "listenerPending": 0,
                "listenerRejected": 0,
                "listenerStale": 0,
                "listenerUnrecoveredFailure": 0
            },
            "messages": [
                "session persistence is active but the dataplane is using an ephemeral secret; configure sessionPersistence.secretKey or secretKeyFile for restart-stable multi-replica deployments"
            ]
        })
    );
    assert_eq!(
        value["warningCategories"],
        serde_json::json!(["session-persistence-ephemeral-secret"])
    );
    assert_eq!(
        value["warningCategoryCounts"],
        serde_json::json!({
            "sessionPersistenceEphemeralSecret": 1,
            "runtimeHttpReloadFailure": 0,
            "runtimeTlsReloadFailure": 0,
            "runtimeStreamReloadFailure": 0,
            "listenerPending": 0,
            "listenerRejected": 0,
            "listenerStale": 0,
            "listenerUnrecoveredFailure": 0
        })
    );
    assert_eq!(value["warnings"].as_array().map(Vec::len), Some(1));
}
