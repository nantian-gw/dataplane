fn assert_composed_overviews_and_warnings(value: &serde_json::Value) {
    assert_eq!(value["summaryOverviews"]["schemaVersion"], 1);
    assert_eq!(
        value["summaryOverviews"]["overviewKeys"],
        serde_json::json!([
            "meta",
            "instance",
            "health",
            "warnings",
            "snapshot",
            "runtime",
            "resources",
            "features",
            "xds",
            "traffic",
            "overload",
            "listenerState",
            "listenerSignals",
        ])
    );
    assert_eq!(value["summaryOverviews"]["meta"], value["metaOverview"]);
    assert_eq!(
        value["summaryOverviews"]["instance"],
        value["instanceOverview"]
    );
    assert_eq!(value["summaryOverviews"]["health"], value["healthOverview"]);
    assert_eq!(
        value["summaryOverviews"]["warnings"],
        value["warningOverview"]
    );
    assert_eq!(
        value["summaryOverviews"]["snapshot"],
        value["snapshotOverview"]
    );
    assert_eq!(
        value["summaryOverviews"]["runtime"],
        value["runtimeOverview"]
    );
    assert_eq!(
        value["summaryOverviews"]["resources"],
        value["resourceOverview"]
    );
    assert_eq!(
        value["summaryOverviews"]["features"],
        value["featureOverview"]
    );
    assert_eq!(value["summaryOverviews"]["xds"], value["xdsOverview"]);
    assert_eq!(
        value["summaryOverviews"]["traffic"],
        value["trafficOverview"]
    );
    assert_eq!(
        value["summaryOverviews"]["overload"],
        value["overloadOverview"]
    );
    assert_eq!(
        value["summaryOverviews"]["listenerState"],
        value["listenerStateOverview"]
    );
    assert_eq!(
        value["summaryOverviews"]["listenerSignals"]["schemaVersion"],
        1
    );
    assert_eq!(
        value["summaryOverviews"]["listenerSignals"]["overviewKeys"],
        serde_json::json!(["bundle", "state",])
    );
    assert_eq!(
        value["summaryOverviews"]["listenerSignals"]["bundle"],
        value["listenerOverviews"]
    );
    assert_eq!(
        value["summaryOverviews"]["listenerSignals"]["state"],
        value["listenerStateOverview"]
    );
    assert_eq!(
        value["warnings"],
        serde_json::json!([
            "latest HTTP listener reload for snapshot v1 failed: web: bind conflict",
            "latest stream listener reload for snapshot v1 failed: passthrough: tcp bind conflict"
        ])
    );
}
