use super::*;

#[tokio::test]
async fn resource_detail_views_expose_runtime_ids() {
    let (app, expected) = runtime_id_app();

    let listener = authorized_json(&app, "/v1/listeners/passthrough").await;
    assert_eq!(
        listener["runtimeId"].as_str(),
        Some(expected.listener.as_str())
    );
    assert_eq!(listener["runtimeRef"]["kind"], "Listener");
    assert_eq!(listener["runtimeRef"]["name"], "passthrough");

    let route = authorized_json(&app, "/v1/routes/tls/default/passthrough").await;
    assert_eq!(route["runtimeId"].as_str(), Some(expected.route.as_str()));
    assert_eq!(route["runtimeRef"]["kind"], "TLSRoute");
    assert_eq!(route["runtimeRef"]["namespace"], "default");
    assert_eq!(route["runtimeRef"]["name"], "passthrough");
    assert_eq!(
        route["ruleRuntimeIds"][0].as_str(),
        Some(expected.rule.as_str())
    );
    assert_eq!(route["ruleRuntimeRefs"][0]["kind"], "TLSRouteRule");
    assert_eq!(route["ruleRuntimeRefs"][0]["namespace"], "default");
    assert_eq!(route["ruleRuntimeRefs"][0]["name"], "passthrough");
    assert_eq!(route["ruleRuntimeRefs"][0]["ruleIndex"], 0);

    let backend = authorized_json(&app, "/v1/backends/default/api:80").await;
    assert_eq!(
        backend["runtimeId"].as_str(),
        Some(expected.backend.as_str())
    );
    assert_eq!(backend["runtimeRef"]["kind"], "Backend");
    assert_eq!(backend["runtimeRef"]["name"], "default/api:80");
    assert_eq!(
        backend["endpoints"][0]["runtimeId"].as_str(),
        Some(expected.endpoint.as_str())
    );
    assert_eq!(backend["endpoints"][0]["runtimeRef"]["kind"], "Endpoint");
    assert_eq!(
        backend["endpoints"][0]["runtimeRef"]["backendName"],
        "default/api:80"
    );
    assert_eq!(
        backend["endpoints"][0]["runtimeRef"]["address"],
        "10.0.0.10"
    );
    assert_eq!(backend["endpoints"][0]["runtimeRef"]["port"], 80);
}

#[tokio::test]
async fn resource_list_views_expose_runtime_ids() {
    let (app, expected) = runtime_id_app();

    let listeners = authorized_json(&app, "/v1/listeners").await;
    let passthrough_listener = listeners
        .as_array()
        .expect("listeners array")
        .iter()
        .find(|listener| listener["name"] == "passthrough")
        .expect("passthrough listener");
    assert_eq!(
        passthrough_listener["runtimeId"].as_str(),
        Some(expected.listener.as_str())
    );
    assert_eq!(passthrough_listener["runtimeRef"]["kind"], "Listener");
    assert_eq!(passthrough_listener["runtimeRef"]["name"], "passthrough");

    let routes = authorized_json(&app, "/v1/routes?kind=tls").await;
    assert_eq!(
        routes["stream"][0]["runtimeId"].as_str(),
        Some(expected.route.as_str())
    );
    assert_eq!(routes["stream"][0]["runtimeRef"]["kind"], "TLSRoute");
    assert_eq!(
        routes["stream"][0]["ruleRuntimeIds"][0].as_str(),
        Some(expected.rule.as_str())
    );
    assert_eq!(
        routes["stream"][0]["ruleRuntimeRefs"][0]["kind"],
        "TLSRouteRule"
    );

    let backends = authorized_json(&app, "/v1/backends?namespace=default&name=api:80").await;
    assert_eq!(
        backends[0]["runtimeId"].as_str(),
        Some(expected.backend.as_str())
    );
    assert_eq!(backends[0]["runtimeRef"]["kind"], "Backend");
    assert_eq!(
        backends[0]["endpoints"][0]["runtimeId"].as_str(),
        Some(expected.endpoint.as_str())
    );
    assert_eq!(
        backends[0]["endpoints"][0]["runtimeRef"]["kind"],
        "Endpoint"
    );
}

#[tokio::test]
async fn resource_list_views_filter_by_runtime_ids() {
    let (app, expected) = runtime_id_app();

    let listeners = authorized_json(
        &app,
        format!("/v1/listeners?runtimeId={}", expected.listener).as_str(),
    )
    .await;
    assert_eq!(listeners.as_array().expect("listeners").len(), 1);
    assert_eq!(listeners[0]["name"], "passthrough");

    let routes = authorized_json(
        &app,
        format!("/v1/routes?runtimeId={}", expected.route).as_str(),
    )
    .await;
    assert!(routes["http"].as_array().expect("http routes").is_empty());
    assert!(routes["grpc"].as_array().expect("grpc routes").is_empty());
    assert_eq!(routes["stream"].as_array().expect("stream routes").len(), 1);
    assert_eq!(routes["stream"][0]["name"], "passthrough");

    let routes = authorized_json(
        &app,
        format!("/v1/routes?ruleRuntimeId={}", expected.rule).as_str(),
    )
    .await;
    assert_eq!(routes["stream"].as_array().expect("stream routes").len(), 1);
    assert_eq!(routes["stream"][0]["name"], "passthrough");

    let backends = authorized_json(
        &app,
        format!("/v1/backends?runtimeId={}", expected.backend).as_str(),
    )
    .await;
    assert_eq!(backends.as_array().expect("backends").len(), 1);
    assert_eq!(backends[0]["name"], "api:80");

    let backends = authorized_json(
        &app,
        format!("/v1/backends?endpointRuntimeId={}", expected.endpoint).as_str(),
    )
    .await;
    assert_eq!(backends.as_array().expect("backends").len(), 1);
    assert_eq!(backends[0]["name"], "api:80");
}

#[derive(Debug)]
struct ExpectedRuntimeIds {
    listener: String,
    route: String,
    rule: String,
    backend: String,
    endpoint: String,
}

fn runtime_id_app() -> (axum::Router, ExpectedRuntimeIds) {
    let mut indexed = fixture_snapshot();
    indexed.rebuild_runtime_indexes();

    let backend_key = "default/api:80";
    let expected = ExpectedRuntimeIds {
        listener: indexed
            .listener_runtime_id("passthrough")
            .expect("listener runtime id")
            .to_string(),
        route: indexed
            .stream_route_runtime_id("TLSRoute", "default", "passthrough")
            .expect("route runtime id")
            .to_string(),
        rule: indexed
            .stream_rule_runtime_id("TLSRoute", "default", "passthrough", 0)
            .expect("rule runtime id")
            .to_string(),
        backend: indexed
            .backend_runtime_id(backend_key)
            .expect("backend runtime id")
            .to_string(),
        endpoint: indexed
            .endpoint_runtime_id(backend_key, &indexed.backends[0].endpoints[0])
            .expect("endpoint runtime id")
            .to_string(),
    };

    let snapshot = Snapshot::shared();
    snapshot.store(Arc::new(indexed));
    let mut config = test_admin_runtime_config();
    config.admin_bearer_token = Some("top-secret".to_string());
    let app = build_router(build_state_with_parts(
        config,
        snapshot,
        RuntimeStats::shared(),
        ClientStats::shared(),
    ));
    (app, expected)
}
