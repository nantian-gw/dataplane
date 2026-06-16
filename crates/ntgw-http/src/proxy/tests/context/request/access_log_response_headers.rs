use super::super::super::request::{
    cache_access_log_connection_fields_from_sources_if_needed,
    cache_access_log_sent_response_headers_from_written_response_if_needed,
    cache_access_log_sent_response_headers_if_needed,
    cache_access_log_upstream_response_headers_if_needed,
    record_access_log_upstream_status_if_needed,
};

#[test]
fn cache_access_log_response_headers_only_captures_named_headers() {
    let mut ctx = RequestContext::default();
    let mut response = ResponseHeader::build(200, None).expect("response");
    response
        .insert_header("content-type", "application/json")
        .expect("content-type");
    response
        .insert_header("server", "orders-upstream")
        .expect("server");
    let access_log = AccessLogOptions {
        enabled: true,
        mode: ntgw_observability::AccessLogMode::Text,
        format: r#"$sent_http_content_type $upstream_http_server"#.to_string(),
        ..AccessLogOptions::default()
    };

    cache_access_log_sent_response_headers_if_needed(
        &mut ctx,
        &response,
        &access_log,
        &BTreeMap::new(),
    );
    cache_access_log_upstream_response_headers_if_needed(
        &mut ctx,
        &response,
        &access_log,
        &BTreeMap::new(),
    );

    assert_eq!(
        ctx.access_log_sent_response_headers,
        BTreeMap::from([("content-type".to_string(), "application/json".to_string())])
    );
    assert_eq!(
        ctx.access_log_upstream_response_headers,
        BTreeMap::from([("server".to_string(), "orders-upstream".to_string())])
    );
}

#[test]
fn record_access_log_upstream_status_tracks_all_observed_statuses() {
    let mut ctx = RequestContext::default();
    let access_log = AccessLogOptions {
        enabled: true,
        mode: ntgw_observability::AccessLogMode::Text,
        format: "$upstream_status".to_string(),
        ..AccessLogOptions::default()
    };

    record_access_log_upstream_status_if_needed(&mut ctx, 502, &access_log, &BTreeMap::new());
    record_access_log_upstream_status_if_needed(&mut ctx, 200, &access_log, &BTreeMap::new());

    assert_eq!(ctx.access_log_upstream_statuses, vec![502, 200]);
}

#[test]
fn cache_access_log_connection_fields_records_https_and_client_port_for_tls_requests() {
    let mut ctx = RequestContext::default();
    let access_log = AccessLogOptions {
        enabled: true,
        mode: ntgw_observability::AccessLogMode::Text,
        format: "$scheme $remote_port".to_string(),
        ..AccessLogOptions::default()
    };

    cache_access_log_connection_fields_from_sources_if_needed(
        &mut ctx,
        &access_log,
        &BTreeMap::new(),
        true,
        Some(54432),
        Some(61234),
    );

    assert_eq!(ctx.access_log_scheme, "https");
    assert_eq!(ctx.access_log_remote_port, Some(54432));
}

#[test]
fn cache_access_log_connection_fields_records_http_for_non_tls_requests() {
    let mut ctx = RequestContext::default();
    let access_log = AccessLogOptions {
        enabled: true,
        mode: ntgw_observability::AccessLogMode::Text,
        format: "$scheme".to_string(),
        ..AccessLogOptions::default()
    };

    cache_access_log_connection_fields_from_sources_if_needed(
        &mut ctx,
        &access_log,
        &BTreeMap::new(),
        false,
        Some(54432),
        Some(61234),
    );

    assert_eq!(ctx.access_log_scheme, "http");
    assert!(ctx.access_log_remote_port.is_none());
}

#[test]
fn cache_access_log_connection_fields_falls_back_to_digest_peer_port() {
    let mut ctx = RequestContext::default();
    let access_log = AccessLogOptions {
        enabled: true,
        mode: ntgw_observability::AccessLogMode::Text,
        format: "$remote_port".to_string(),
        ..AccessLogOptions::default()
    };

    cache_access_log_connection_fields_from_sources_if_needed(
        &mut ctx,
        &access_log,
        &BTreeMap::new(),
        false,
        None,
        Some(61234),
    );

    assert_eq!(ctx.access_log_remote_port, Some(61234));
}

#[test]
fn cache_access_log_connection_fields_leaves_remote_port_empty_without_inet_sources() {
    let mut ctx = RequestContext::default();
    let access_log = AccessLogOptions {
        enabled: true,
        mode: ntgw_observability::AccessLogMode::Text,
        format: "$remote_port".to_string(),
        ..AccessLogOptions::default()
    };

    cache_access_log_connection_fields_from_sources_if_needed(
        &mut ctx,
        &access_log,
        &BTreeMap::new(),
        false,
        None,
        None,
    );

    assert_eq!(ctx.access_log_remote_port, None);
    assert!(ctx.access_log_scheme.is_empty());
}

#[test]
fn cache_access_log_sent_response_headers_fallback_overwrites_with_written_response() {
    let mut ctx = RequestContext {
        access_log_sent_response_headers: BTreeMap::from([(
            "server".to_string(),
            "stale-provisional".to_string(),
        )]),
        ..RequestContext::default()
    };
    let mut response = ResponseHeader::build(503, None).expect("response");
    response.insert_header("server", "pingora").expect("server");
    response
        .insert_header("cache-control", "private, no-store")
        .expect("cache-control");
    let access_log = AccessLogOptions {
        enabled: true,
        mode: ntgw_observability::AccessLogMode::Text,
        format: "$sent_http_server $sent_http_cache_control".to_string(),
        ..AccessLogOptions::default()
    };

    cache_access_log_sent_response_headers_from_written_response_if_needed(
        &mut ctx,
        Some(&response),
        &access_log,
        &BTreeMap::new(),
    );

    assert_eq!(
        ctx.access_log_sent_response_headers,
        BTreeMap::from([
            ("cache-control".to_string(), "private, no-store".to_string()),
            ("server".to_string(), "pingora".to_string()),
        ])
    );
}
