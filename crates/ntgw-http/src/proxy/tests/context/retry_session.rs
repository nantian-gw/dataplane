use super::*;

#[test]
fn falls_back_to_first_backend() {
    let snapshot = Snapshot {
        backends: vec![BackendCluster {
            ai_service: None,
            token_policy: None,
            name: "echo:8080".to_string(),
            namespace: "default".to_string(),
            protocol: "HTTP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "127.0.0.1".to_string(),
                port: 8080,
                healthy: true,
            }],
            wasm_plugin: None,
        }],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_backend(&RequestMeta::new(None, "/", "GET", BTreeMap::new()))
        .expect("backend");
    assert_eq!(selected.backend.port, 8080);
}

#[test]
fn session_resolution_cache_reuses_resolved_cookie_session() {
    let manager = SessionManager::new(
        SessionPersistenceOptions::build(Some(b"0123456789abcdef0123456789abcdef".to_vec()), None)
            .expect("session options"),
    );
    let policy = SessionPersistence {
        session_name: "ntgw-http-session".to_string(),
        session_type: "Cookie".to_string(),
        absolute_timeout: None,
        idle_timeout: None,
        cookie: Some(CookieConfig {
            lifetime_type: "Permanent".to_string(),
        }),
    };
    let selected = SelectedBackend {
        route_kind: RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "listener".to_string(),
        listener_protocol: "HTTP".to_string(),
        backend: BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/echo:8080".to_string(),
        filters: Vec::new(),
        matched_http_path: None,
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
    };
    let mut response = ResponseHeader::build(200, None).expect("response");
    manager
        .write_response_session(&mut response, &policy, &selected, None)
        .expect("write session");
    let cookie_value = response
        .headers
        .get("set-cookie")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .and_then(|value| value.split_once('='))
        .map(|(_, value)| value.to_string())
        .expect("cookie value");
    let headers = BTreeMap::from([(
        "cookie".to_string(),
        vec![format!("{}={cookie_value}", policy.session_name)],
    )]);

    let cache = SessionResolutionCache::new(&manager, &headers);
    let first = cache.resolved_session(&policy).expect("first session");
    let second = cache.resolved_session(&policy).expect("second session");

    assert_eq!(first.target.backend_name, "default/echo:8080");
    assert_eq!(second.target.backend_name, first.target.backend_name);
    assert_eq!(
        cache.resolve_target(&policy).expect("target").backend_name,
        "default/echo:8080"
    );
}

#[test]
fn retry_completed_successfully_requires_retry_and_non_5xx_status() {
    let mut ctx = RequestContext {
        retry_attempts: 1,
        status: 200,
        ..RequestContext::default()
    };
    assert!(retry_completed_successfully(&ctx));

    ctx.status = 404;
    assert!(retry_completed_successfully(&ctx));

    ctx.status = 503;
    assert!(!retry_completed_successfully(&ctx));

    ctx.retry_attempts = 0;
    ctx.status = 200;
    assert!(!retry_completed_successfully(&ctx));
}

#[test]
fn response_is_retryable_only_for_configured_status_codes() {
    let mut selected = sample_selected_backend("127.0.0.1", "default/echo:8080");
    selected.retry = Some(RetryPolicy {
        codes: vec![500, 503],
        attempts: 2,
        backoff: None,
    });
    let ctx = RequestContext {
        method: "GET".to_string(),
        selected_backend: Some(Arc::new(selected)),
        ..RequestContext::default()
    };

    assert!(response_is_retryable(&ctx, 503));
    assert!(!response_is_retryable(&ctx, 404));
}

#[test]
fn response_is_retryable_requires_replayable_http_method() {
    let mut selected = sample_selected_backend("127.0.0.1", "default/echo:8080");
    selected.retry = Some(RetryPolicy {
        codes: vec![503],
        attempts: 2,
        backoff: None,
    });
    let selected = Arc::new(selected);
    let mut ctx = RequestContext {
        method: "GET".to_string(),
        selected_backend: Some(selected),
        ..RequestContext::default()
    };

    assert!(response_is_retryable(&ctx, 503));

    ctx.method = "POST".to_string();
    assert!(!response_is_retryable(&ctx, 503));

    ctx.method = "PATCH".to_string();
    assert!(!response_is_retryable(&ctx, 503));

    ctx.method.clear();
    assert!(!response_is_retryable(&ctx, 503));
}

#[test]
fn retry_backoff_applies_only_after_the_first_retry() {
    let mut ctx = RequestContext {
        retry_backoff: Some(std::time::Duration::from_millis(25)),
        ..RequestContext::default()
    };

    assert_eq!(retry_backoff(&ctx), None);
    ctx.retry_attempts = 1;
    assert_eq!(
        retry_backoff(&ctx),
        Some(std::time::Duration::from_millis(25))
    );
}
