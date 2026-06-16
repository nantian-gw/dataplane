use super::super::super::request::{
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
