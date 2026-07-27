use std::sync::OnceLock;

use http::header::{HeaderName, HeaderValue};
use opentelemetry::propagation::{Extractor, Injector};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use pingora::http::RequestHeader;
use std::collections::BTreeMap;
use tracing::Span;

static TRACE_CONTEXT_PROPAGATOR: OnceLock<TraceContextPropagator> = OnceLock::new();

pub(crate) fn trace_context_propagator() -> &'static TraceContextPropagator {
    TRACE_CONTEXT_PROPAGATOR.get_or_init(TraceContextPropagator::new)
}

pub(crate) fn record_span_string(span: &Span, field: &'static str, value: &str) {
    if !value.is_empty() && value != "-" {
        span.record(field, value);
    }
}

pub(crate) struct TraceHeaderExtractor<'a> {
    pub(crate) headers: &'a BTreeMap<String, Vec<String>>,
}

impl Extractor for TraceHeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers
            .get(key)
            .or_else(|| self.headers.get(&key.to_ascii_lowercase()))
            .and_then(|values| values.first())
            .map(String::as_str)
    }

    fn keys(&self) -> Vec<&str> {
        self.headers.keys().map(String::as_str).collect()
    }
}

pub(crate) struct RequestHeaderExtractor<'a> {
    pub(crate) request: &'a RequestHeader,
}

impl Extractor for RequestHeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.request
            .headers
            .get(key)
            .and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.request
            .headers
            .keys()
            .map(HeaderName::as_str)
            .collect()
    }
}

pub(crate) struct RequestHeaderInjector<'a> {
    pub(crate) request: &'a mut RequestHeader,
}

impl Injector for RequestHeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        let Ok(header_name) = HeaderName::from_bytes(key.as_bytes()) else {
            return;
        };
        let Ok(header_value) = HeaderValue::from_str(&value) else {
            return;
        };
        self.request.headers.insert(header_name, header_value);
    }
}
