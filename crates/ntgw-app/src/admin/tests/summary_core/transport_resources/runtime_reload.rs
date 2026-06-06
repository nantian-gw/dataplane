fn assert_runtime_reload_overview(value: &serde_json::Value) {
    assert_eq!(value["runtimeHttpListenerReloadFailures"], 1);
    assert_eq!(value["runtimeHttpLastReloadAttemptVersion"], "v1");
    assert_eq!(value["runtimeHttpLastGoodReloadVersion"], "");
    assert_eq!(value["runtimeHttpLastReloadFailureVersion"], "v1");
    assert_eq!(value["runtimeHttpLastReloadFailureListener"], "web");
    assert_eq!(
        value["runtimeHttpLastReloadFailureMessage"],
        "bind conflict"
    );
    assert_eq!(
        value["runtimeHttpCurrentFailures"],
        serde_json::json!([{ "listener": "web", "message": "bind conflict" }])
    );
    assert_eq!(value["runtimeHttpTlsAssetReuses"], 2);
    assert_eq!(value["runtimeStreamListenerReloadFailures"], 1);
    assert_eq!(value["runtimeStreamLastReloadAttemptVersion"], "v1");
    assert_eq!(value["runtimeStreamLastGoodReloadVersion"], "");
    assert_eq!(value["runtimeStreamLastReloadFailureVersion"], "v1");
    assert_eq!(
        value["runtimeStreamLastReloadFailureListener"],
        "passthrough"
    );
    assert_eq!(
        value["runtimeStreamLastReloadFailureMessage"],
        "tcp bind conflict"
    );
    assert_eq!(
        value["runtimeStreamCurrentFailures"],
        serde_json::json!([{ "listener": "passthrough", "message": "tcp bind conflict" }])
    );
}
