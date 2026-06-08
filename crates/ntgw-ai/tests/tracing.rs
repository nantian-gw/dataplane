use ntgw_ai::observability::tracing::AITracer;
use opentelemetry::trace::{Span, TraceContextExt, Tracer, TracerProvider as _};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider, SimpleSpanProcessor};
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[test]
fn test_span_attributes() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
        .build();
    let tracer = provider.tracer("test-service");

    let mut span =
        tracer.start_with_context("inference".to_string(), &opentelemetry::Context::current());
    span.set_attribute(opentelemetry::KeyValue::new("ai.model", "gpt-4"));
    span.set_attribute(opentelemetry::KeyValue::new("ai.format", "openai"));
    span.set_attribute(opentelemetry::KeyValue::new("ai.stream", true));

    let ext_provider = SdkTracerProvider::builder().build();
    let ext_tracer = ext_provider.tracer("ext-test");
    let otel_parent =
        ext_tracer.start_with_context("parent".to_string(), &opentelemetry::Context::current());
    let otel_cx = opentelemetry::Context::current_with_span(otel_parent);
    let ots = tracing::info_span!("test_span_attributes");
    let _ = ots.set_parent(otel_cx);
    drop(ext_tracer);
    drop(ext_provider);
    span.end();

    drop(tracer);
    drop(provider);

    let spans = exporter
        .get_finished_spans()
        .expect("should have exported spans");
    assert_eq!(spans.len(), 1, "expected exactly one exported span");

    let exported = &spans[0];
    let has_model = exported
        .attributes
        .iter()
        .any(|kv| kv.key.as_str() == "ai.model" && kv.value.as_str() == "gpt-4");
    let has_format = exported
        .attributes
        .iter()
        .any(|kv| kv.key.as_str() == "ai.format" && kv.value.as_str() == "openai");
    let has_stream = exported
        .attributes
        .iter()
        .any(|kv| kv.key.as_str() == "ai.stream" && kv.value.as_str() == "true");

    assert!(has_model, "missing ai.model attribute");
    assert!(has_format, "missing ai.format attribute");
    assert!(has_stream, "missing ai.stream attribute");
}

#[test]
fn test_span_token_attributes() {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_span_processor(SimpleSpanProcessor::new(exporter.clone()))
        .build();
    let tracer = provider.tracer("test-service");

    let mut span =
        tracer.start_with_context("inference".to_string(), &opentelemetry::Context::current());
    span.set_attribute(opentelemetry::KeyValue::new("ai.prompt_tokens", 42i64));
    span.set_attribute(opentelemetry::KeyValue::new("ai.completion_tokens", 128i64));

    let ext_provider = SdkTracerProvider::builder().build();
    let ext_tracer = ext_provider.tracer("ext-test");
    let otel_parent = ext_tracer.start_with_context(
        "parent_tokens".to_string(),
        &opentelemetry::Context::current(),
    );
    let otel_cx = opentelemetry::Context::current_with_span(otel_parent);
    let ots = tracing::info_span!("test_tokens");
    let _ = ots.set_parent(otel_cx);

    drop(ext_tracer);
    drop(ext_provider);
    span.end();

    drop(tracer);
    drop(provider);

    let spans = exporter
        .get_finished_spans()
        .expect("should have exported spans");
    assert_eq!(spans.len(), 1);

    let exported = &spans[0];
    let has_prompt = exported
        .attributes
        .iter()
        .any(|kv| kv.key.as_str() == "ai.prompt_tokens" && kv.value.as_str() == "42");
    let has_completion = exported
        .attributes
        .iter()
        .any(|kv| kv.key.as_str() == "ai.completion_tokens" && kv.value.as_str() == "128");

    assert!(has_prompt, "missing prompt_tokens attribute");
    assert!(has_completion, "missing completion_tokens attribute");
}

#[test]
fn test_noop_tracer() {
    #[allow(clippy::unwrap_used)]
    let tracer = AITracer::new("noop-test", "").unwrap();

    let span = tracer.start_span("noop_inference", "gpt-3.5", "anthropic", false);
    assert_eq!(span.name, "noop_inference");

    tracer.end_span(span, 10, 20);

    tracer.shutdown();

    // Verify no panic on second span
    let span2 = tracer.start_span("noop_second", "claude-3", "openai", true);
    tracer.end_span(span2, 0, 0);

    let ots = tracing::info_span!("noop_test");
    let _cx = ots.context();
}
