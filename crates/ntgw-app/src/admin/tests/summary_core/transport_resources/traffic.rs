fn assert_traffic_overview(value: &serde_json::Value) {
    assert_eq!(value["trafficTotalEvents"], 0);
    assert_eq!(value["trafficRetriedSuccessEvents"], 0);
    assert_eq!(value["trafficRetryRate"], 0.0);
    assert_eq!(value["trafficFailoverSuccessRate"], 0.0);
    assert_eq!(value["trafficUpstreamPoolHits"], 0);
    assert_eq!(value["trafficUpstreamPoolMisses"], 0);
    assert_eq!(value["trafficUpstreamPoolHitRatio"], 0.0);
    assert_eq!(value["trafficUpstreamConnectLatencyMsAvg"], 0.0);
    assert_eq!(value["trafficUpstreamConnectLatencyMsMax"], 0);
    assert_eq!(
        value["trafficOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "summary": {
                "counts": {
                    "totalEvents": 0,
                    "retriedSuccessEvents": 0,
                    "retriedEvents": 0,
                    "retryAttempts": 0,
                    "bytesReceived": 0,
                    "bytesSent": 0,
                    "upstreamPoolHits": 0,
                    "upstreamPoolMisses": 0,
                },
                "status": {
                    "retryRate": 0.0,
                    "failoverSuccessRate": 0.0,
                    "upstreamPoolHitRatio": 0.0,
                    "maxLatencyMs": 0,
                    "upstreamConnectAvgMs": 0.0,
                    "upstreamConnectMaxMs": 0,
                }
            },
            "events": {
                "total": 0,
                "retriedSuccess": 0,
                "retried": 0,
                "retryAttempts": 0,
            },
            "bytes": {
                "received": 0,
                "sent": 0,
            },
            "rates": {
                "retry": 0.0,
                "failoverSuccess": 0.0,
                "upstreamPoolHitRatio": 0.0,
            },
            "latencyMs": {
                "max": 0,
                "upstreamConnectAvg": 0.0,
                "upstreamConnectMax": 0,
            },
            "upstreamPool": {
                "hits": 0,
                "misses": 0,
            }
        })
    );
}
