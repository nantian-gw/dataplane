#[test]
fn sampling_respects_extreme_values() {
    let record = AccessLogRecord {
        event: "http_request".to_string(),
        route_name: "orders".to_string(),
        start_time_unix_ms: 42,
        ..AccessLogRecord::default()
    };

    assert!(!should_emit_sample(0.0, &record));
    assert!(should_emit_sample(1.0, &record));
}

#[test]
fn sampling_prefers_runtime_ids_over_display_names() {
    let rate = 0.5;
    let base_without_ids = AccessLogRecord {
        event: "http_request".to_string(),
        listener: "listener-a".to_string(),
        request_id: "request-1".to_string(),
        route_namespace: "default".to_string(),
        route_name: "orders".to_string(),
        backend: "default/orders:8080".to_string(),
        start_time_unix_ms: 42,
        ..AccessLogRecord::default()
    };
    let renamed_without_ids = renamed_record_with_different_string_sample(
        &base_without_ids,
        rate,
    );
    assert_ne!(
        should_emit_sample(rate, &base_without_ids),
        should_emit_sample(rate, &renamed_without_ids),
        "test setup must prove display strings still affect fallback sampling"
    );

    let base_with_ids = with_runtime_ids(base_without_ids);
    let renamed_with_ids = with_runtime_ids(renamed_without_ids);

    assert_eq!(
        should_emit_sample(rate, &base_with_ids),
        should_emit_sample(rate, &renamed_with_ids),
        "runtime IDs should make sampling stable across display-name changes"
    );
}

#[test]
fn write_options_can_sample_before_building_full_record() {
    let options = AccessLogOptions {
        sample_rate: 0.0,
        ..AccessLogOptions::default()
    };
    let key = AccessLogSampleKey {
        event: "http_request",
        listener: "listener-a",
        listener_runtime_id: None,
        request_id: "request-1",
        route_namespace: "default",
        route_name: "orders",
        route_runtime_id: None,
        backend: "default/orders:8080",
        backend_runtime_id: None,
        start_time_unix_ms: 42,
    };

    assert!(resolve_access_log_write_options(&options, &BTreeMap::new(), &key).is_none());
}

#[test]
fn write_options_borrow_base_when_route_annotations_do_not_override_access_log() {
    let options = AccessLogOptions {
        sample_rate: 1.0,
        ..AccessLogOptions::default()
    };
    let mut annotations = BTreeMap::new();
    annotations.insert(
        "gateway.nantian.dev/access-log-extra-0".to_string(),
        "ignored".to_string(),
    );
    annotations.insert("example.com/unrelated".to_string(), "ignored".to_string());
    let key = AccessLogSampleKey {
        event: "http_request",
        listener: "listener-a",
        listener_runtime_id: None,
        request_id: "request-1",
        route_namespace: "default",
        route_name: "orders",
        route_runtime_id: None,
        backend: "default/orders:8080",
        backend_runtime_id: None,
        start_time_unix_ms: 42,
    };

    let resolved = resolve_access_log_write_options(&options, &annotations, &key);

    assert!(
        matches!(resolved, Some(std::borrow::Cow::Borrowed(_))),
        "route annotations without access-log overrides should not clone base log options"
    );
}

#[test]
fn write_options_sampling_prefers_runtime_ids_over_display_names() {
    let rate = 0.5;
    let options = AccessLogOptions {
        sample_rate: rate,
        ..AccessLogOptions::default()
    };
    let base_without_ids = AccessLogSampleKey {
        event: "http_request",
        listener: "listener-a",
        listener_runtime_id: None,
        request_id: "request-1",
        route_namespace: "default",
        route_name: "orders",
        route_runtime_id: None,
        backend: "default/orders:8080",
        backend_runtime_id: None,
        start_time_unix_ms: 42,
    };
    let renamed_without_ids = renamed_key_with_different_string_sample(
        &options,
        &base_without_ids,
    );
    assert_ne!(
        resolve_access_log_write_options(&options, &BTreeMap::new(), &base_without_ids).is_some(),
        resolve_access_log_write_options(&options, &BTreeMap::new(), &renamed_without_ids).is_some(),
        "test setup must prove display strings still affect fallback sampling"
    );

    let base_with_ids = with_key_runtime_ids(base_without_ids);
    let renamed_with_ids = with_key_runtime_ids(renamed_without_ids);

    assert_eq!(
        resolve_access_log_write_options(&options, &BTreeMap::new(), &base_with_ids).is_some(),
        resolve_access_log_write_options(&options, &BTreeMap::new(), &renamed_with_ids).is_some(),
        "runtime IDs should make write-option sampling stable across display-name changes"
    );
}

fn renamed_record_with_different_string_sample(
    base: &AccessLogRecord,
    rate: f64,
) -> AccessLogRecord {
    let base_decision = should_emit_sample(rate, base);
    for index in 0..10_000 {
        let mut renamed = base.clone();
        renamed.listener = format!("listener-renamed-{index}");
        renamed.route_namespace = format!("namespace-renamed-{index}");
        renamed.route_name = format!("route-renamed-{index}");
        renamed.backend = format!("namespace-renamed-{index}/backend-renamed-{index}:8080");
        if should_emit_sample(rate, &renamed) != base_decision {
            return renamed;
        }
    }
    panic!("could not find display names with a different sampling decision");
}

fn with_runtime_ids(mut record: AccessLogRecord) -> AccessLogRecord {
    record.listener_runtime_id = Some("0000000000000001".to_string());
    record.route_runtime_id = Some("0000000000000002".to_string());
    record.backend_runtime_id = Some("0000000000000003".to_string());
    record
}

fn renamed_key_with_different_string_sample<'a>(
    options: &AccessLogOptions,
    base: &AccessLogSampleKey<'a>,
) -> AccessLogSampleKey<'static> {
    let base_decision =
        resolve_access_log_write_options(options, &BTreeMap::new(), base).is_some();
    for index in 0..10_000 {
        let renamed = AccessLogSampleKey {
            event: "http_request",
            listener: Box::leak(format!("listener-renamed-{index}").into_boxed_str()),
            listener_runtime_id: base.listener_runtime_id,
            request_id: "request-1",
            route_namespace: Box::leak(format!("namespace-renamed-{index}").into_boxed_str()),
            route_name: Box::leak(format!("route-renamed-{index}").into_boxed_str()),
            route_runtime_id: base.route_runtime_id,
            backend: Box::leak(
                format!("namespace-renamed-{index}/backend-renamed-{index}:8080")
                    .into_boxed_str(),
            ),
            backend_runtime_id: base.backend_runtime_id,
            start_time_unix_ms: base.start_time_unix_ms,
        };
        if resolve_access_log_write_options(options, &BTreeMap::new(), &renamed).is_some()
            != base_decision
        {
            return renamed;
        }
    }
    panic!("could not find display names with a different sampling decision");
}

fn with_key_runtime_ids(mut key: AccessLogSampleKey<'_>) -> AccessLogSampleKey<'_> {
    key.listener_runtime_id = Some(1);
    key.route_runtime_id = Some(2);
    key.backend_runtime_id = Some(3);
    key
}
