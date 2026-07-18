use ntgw_ai::observability::metrics::AIMetrics;
use prometheus::Registry;

fn gather_metric_value(registry: &Registry, name: &str, labels: &[(&str, &str)]) -> f64 {
    let families = registry.gather();
    for family in &families {
        if family.name() == name {
            for m in family.get_metric() {
                if labels_match(m.get_label(), labels)
                    && family.get_field_type() == prometheus::proto::MetricType::COUNTER {
                        return m.get_counter().value();
                    }
            }
        }
    }
    // For counters that may not have been hit, 0.0 is fine.
    0.0
}

fn gather_histogram_count(registry: &Registry, name: &str, labels: &[(&str, &str)]) -> u64 {
    let families = registry.gather();
    for family in &families {
        if family.name() == name {
            for m in family.get_metric() {
                if labels_match(m.get_label(), labels) {
                    return m.get_histogram().get_sample_count();
                }
            }
        }
    }
    0
}

fn labels_match(got: &[prometheus::proto::LabelPair], want: &[(&str, &str)]) -> bool {
    if got.len() != want.len() {
        return false;
    }
    for w in want {
        if !got.iter().any(|lp| lp.name() == w.0 && lp.value() == w.1) {
            return false;
        }
    }
    true
}

#[test]
fn test_token_metrics_emission() {
    let registry = Registry::new();
    let metrics = AIMetrics::new(&registry).expect("register AI metrics");

    metrics.record_tokens("gpt-4", 150, 80);

    let prompt_val = gather_metric_value(
        &registry,
        "ai_tokens_total",
        &[("model", "gpt-4"), ("direction", "prompt")],
    );
    let completion_val = gather_metric_value(
        &registry,
        "ai_tokens_total",
        &[("model", "gpt-4"), ("direction", "completion")],
    );

    assert_eq!(prompt_val, 150.0, "prompt token count should be 150");
    assert_eq!(completion_val, 80.0, "completion token count should be 80");
}

#[test]
fn test_request_duration_histogram() {
    let registry = Registry::new();
    let metrics = AIMetrics::new(&registry).expect("register AI metrics");

    metrics.record_request("claude-3", "anthropic", "success", 0.75);

    let count = gather_histogram_count(
        &registry,
        "ai_request_duration_seconds",
        &[("model", "claude-3"), ("provider", "")],
    );
    assert_eq!(count, 1, "duration histogram should have 1 observation");
}

#[test]
fn test_format_error_counter() {
    let registry = Registry::new();
    let metrics = AIMetrics::new(&registry).expect("register AI metrics");

    metrics.record_format_error("openai", "parse_error");
    metrics.record_format_error("openai", "parse_error");
    metrics.record_format_error("anthropic", "unsupported_field");

    let openai_val = gather_metric_value(
        &registry,
        "ai_format_errors_total",
        &[("format", "openai"), ("reason", "parse_error")],
    );
    let anthropic_val = gather_metric_value(
        &registry,
        "ai_format_errors_total",
        &[("format", "anthropic"), ("reason", "unsupported_field")],
    );

    assert_eq!(openai_val, 2.0, "openai parse_error count should be 2");
    assert_eq!(
        anthropic_val, 1.0,
        "anthropic unsupported_field count should be 1"
    );
}

#[test]
fn test_stream_event_counters() {
    let registry = Registry::new();
    let metrics = AIMetrics::new(&registry).expect("register AI metrics");

    metrics.record_stream_event("gpt-4", "content");
    metrics.record_stream_event("gpt-4", "content");
    metrics.record_stream_event("gpt-4", "done");
    metrics.record_stream_event("gpt-4", "error");

    let content_val = gather_metric_value(
        &registry,
        "ai_stream_events_total",
        &[("model", "gpt-4"), ("event_type", "content")],
    );
    let done_val = gather_metric_value(
        &registry,
        "ai_stream_events_total",
        &[("model", "gpt-4"), ("event_type", "done")],
    );
    let error_val = gather_metric_value(
        &registry,
        "ai_stream_events_total",
        &[("model", "gpt-4"), ("event_type", "error")],
    );

    assert_eq!(content_val, 2.0);
    assert_eq!(done_val, 1.0);
    assert_eq!(error_val, 1.0);
}

#[test]
fn test_backend_error_counter() {
    let registry = Registry::new();
    let metrics = AIMetrics::new(&registry).expect("register AI metrics");

    metrics.record_backend_error("gpt-4", "503");
    metrics.record_backend_error("gpt-4", "503");
    metrics.record_backend_error("gpt-4", "429");

    let v503 = gather_metric_value(
        &registry,
        "ai_backend_errors_total",
        &[("model", "gpt-4"), ("status_code", "503")],
    );
    let v429 = gather_metric_value(
        &registry,
        "ai_backend_errors_total",
        &[("model", "gpt-4"), ("status_code", "429")],
    );

    assert_eq!(v503, 2.0);
    assert_eq!(v429, 1.0);
}

#[test]
fn test_langfuse_counter() {
    let registry = Registry::new();
    let metrics = AIMetrics::new(&registry).expect("register AI metrics");

    metrics
        .langfuse_ingestions
        .with_label_values(&["trace"])
        .inc();
    metrics
        .langfuse_ingestions
        .with_label_values(&["trace"])
        .inc();
    metrics
        .langfuse_ingestions
        .with_label_values(&["generation"])
        .inc();

    let trace_val = gather_metric_value(
        &registry,
        "ai_langfuse_ingestions_total",
        &[("ingestion_type", "trace")],
    );
    let gen_val = gather_metric_value(
        &registry,
        "ai_langfuse_ingestions_total",
        &[("ingestion_type", "generation")],
    );

    assert_eq!(trace_val, 2.0);
    assert_eq!(gen_val, 1.0);
}
