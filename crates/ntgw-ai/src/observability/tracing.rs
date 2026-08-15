use opentelemetry::{
    Context, ContextGuard, KeyValue,
    baggage::{Baggage, BaggageExt},
    trace::{Span, Tracer, TracerProvider as _},
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{Resource, trace as sdk_trace};
use std::time::Instant;

/// Tracer for AI gateway observability via OpenTelemetry.
pub struct AITracer {
    tracer: sdk_trace::Tracer,
    provider: sdk_trace::SdkTracerProvider,
}

/// A span representing an AI inference operation.
#[derive(Debug)]
pub struct AISpan {
    pub name: String,
    pub start: Instant,
    span: sdk_trace::Span,
}

impl AITracer {
    /// Create a new AITracer.
    ///
    /// If `exporter_endpoint` is empty, a noop tracer is created that does not export spans.
    /// Otherwise, an OTLP gRPC exporter is configured with batch processing.
    pub fn new(service_name: &str, exporter_endpoint: &str) -> Result<Self, anyhow::Error> {
        if exporter_endpoint.is_empty() {
            let provider = sdk_trace::SdkTracerProvider::builder().build();
            let tracer = provider.tracer(service_name.to_string());
            Ok(Self { tracer, provider })
        } else {
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(exporter_endpoint.to_string())
                .build()
                .map_err(|e| anyhow::anyhow!("failed to create OTLP span exporter: {e}"))?;

            let resource = Resource::builder()
                .with_service_name(service_name.to_string())
                .build();

            let provider = sdk_trace::SdkTracerProvider::builder()
                .with_batch_exporter(exporter)
                .with_resource(resource)
                .build();

            let tracer = provider.tracer(service_name.to_string());
            Ok(Self { tracer, provider })
        }
    }

    /// Start a new AI span with model, format, and stream attributes.
    pub fn start_span(&self, name: &str, model: &str, format: &str, stream: bool) -> AISpan {
        let mut span = self
            .tracer
            .start_with_context(name.to_string(), &Context::current());
        span.set_attribute(KeyValue::new("ai.model", model.to_string()));
        span.set_attribute(KeyValue::new("ai.format", format.to_string()));
        span.set_attribute(KeyValue::new("ai.stream", stream));
        AISpan {
            name: name.to_string(),
            start: Instant::now(),
            span,
        }
    }

    /// Start a child span linked to the current trace context.
    ///
    /// Records `ai.model` and `ai.format` attributes. Use this for
    /// downstream operations that need to be linked to parent spans
    /// (e.g., upstream request timing, rate-limit checks).
    pub fn start_child_span(&self, name: &str, model: &str, format: &str) -> AISpan {
        let mut span = self
            .tracer
            .start_with_context(name.to_string(), &Context::current());
        span.set_attribute(KeyValue::new("ai.model", model.to_string()));
        span.set_attribute(KeyValue::new("ai.format", format.to_string()));
        AISpan {
            name: name.to_string(),
            start: Instant::now(),
            span,
        }
    }

    /// Record upstream timing metrics on a span and end it.
    ///
    /// Sets `upstream.connect_ms`, `upstream.ttfb_ms`, and
    /// `upstream.total_ms` attributes before ending the span.
    pub fn record_upstream_timing(
        &self,
        mut span_obj: AISpan,
        connect_ms: u64,
        ttfb_ms: u64,
        total_ms: u64,
    ) {
        span_obj
            .span
            .set_attribute(KeyValue::new("upstream.connect_ms", connect_ms as i64));
        span_obj
            .span
            .set_attribute(KeyValue::new("upstream.ttfb_ms", ttfb_ms as i64));
        span_obj
            .span
            .set_attribute(KeyValue::new("upstream.total_ms", total_ms as i64));
        span_obj.span.end();
    }

    /// End an AI span, recording prompt and completion token counts.
    pub fn end_span(&self, mut span_obj: AISpan, prompt_tokens: u64, completion_tokens: u64) {
        span_obj
            .span
            .set_attribute(KeyValue::new("ai.prompt_tokens", prompt_tokens as i64));
        span_obj.span.set_attribute(KeyValue::new(
            "ai.completion_tokens",
            completion_tokens as i64,
        ));
        span_obj.span.end();
    }

    /// Set a baggage entry on the current OpenTelemetry context.
    ///
    /// Preserves existing baggage entries and attaches the modified context.
    /// Returns a [`ContextGuard`] that must be held for the duration where
    /// the baggage should be active. When dropped, the previous context is
    /// restored.
    pub fn set_baggage(&self, key: &str, value: &str) -> Result<ContextGuard, anyhow::Error> {
        let cx = Context::current();
        let existing = cx.baggage();

        let mut bag = Baggage::new();
        for entry in existing.iter() {
            let _ = bag.insert(entry.0.clone(), entry.1.0.to_string());
        }
        let _ = bag.insert(key.to_string(), value.to_string());
        Ok(Context::current_with_baggage(bag).attach())
    }

    /// Get a baggage value from the current OpenTelemetry context.
    #[must_use]
    pub fn get_baggage(&self, key: &str) -> Option<String> {
        Context::current().baggage().get(key).map(|v| v.to_string())
    }

    /// Add an event with attributes to an active span.
    ///
    /// Use this to record milestone events within an AI span, such as
    /// token limit checks, content safety evaluations, or model routing
    /// decisions.
    pub fn add_span_event(&self, span: &mut AISpan, name: &str, attributes: Vec<KeyValue>) {
        span.span.add_event(name.to_string(), attributes);
    }

    /// Shut down the tracer provider, flushing any pending spans.
    pub fn shutdown(&self) {
        let _ = self.provider.force_flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_baggage() {
        #[allow(clippy::unwrap_used)]
        let tracer = AITracer::new("test-svc", "").unwrap();
        #[allow(clippy::unwrap_used)]
        let _guard = tracer.set_baggage("tenant-id", "acme-corp").unwrap();
        assert_eq!(
            tracer.get_baggage("tenant-id"),
            Some("acme-corp".to_string())
        );
        assert_eq!(tracer.get_baggage("nonexistent"), None);
    }

    #[test]
    fn test_baggage_preserves_existing_entries() {
        #[allow(clippy::unwrap_used)]
        let tracer = AITracer::new("test-svc", "").unwrap();
        #[allow(clippy::unwrap_used)]
        let g1 = tracer.set_baggage("k1", "v1").unwrap();
        #[allow(clippy::unwrap_used)]
        let g2 = tracer.set_baggage("k2", "v2").unwrap();
        assert_eq!(tracer.get_baggage("k1"), Some("v1".to_string()));
        assert_eq!(tracer.get_baggage("k2"), Some("v2".to_string()));
        drop(g2);
        drop(g1);
    }

    #[test]
    fn test_add_span_event() {
        #[allow(clippy::unwrap_used)]
        let tracer = AITracer::new("test-svc", "").unwrap();
        let mut span = tracer.start_span("test-op", "gpt-4", "openai", false);
        tracer.add_span_event(
            &mut span,
            "rate-limit-check",
            vec![
                KeyValue::new("limit", 100_i64),
                KeyValue::new("remaining", 99_i64),
            ],
        );
        span.span.end();
        // No assertion on noop tracer — verifies method compiles and runs without panic.
    }
}
