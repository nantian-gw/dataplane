use prometheus::{
    HistogramOpts, HistogramVec, IntCounter, IntCounterVec, Opts, Registry,
    register_histogram_vec_with_registry, register_int_counter_vec_with_registry,
    register_int_counter_with_registry,
};

/// Container for all AI Gateway Prometheus metrics, registered with the
/// provided [`Registry`] via [`AIMetrics::new`].
pub struct AIMetrics {
    // ── CounterVecs ──────────────────────────────────────────────
    /// Total token counts. Labels: `model`, `direction` (`prompt`|`completion`)
    pub tokens_total: IntCounterVec,
    /// Total AI request counts. Labels: `model`, `format`, `status`
    pub requests_total: IntCounterVec,
    /// Streamed event counts. Labels: `model`, `event_type` (`content`|`done`|`error`)
    pub stream_events_total: IntCounterVec,
    /// Backend (upstream LLM provider) error counts. Labels: `model`, `status_code`
    pub backend_errors_total: IntCounterVec,
    /// Format-related (serialisation / deserialisation) error counts.
    /// Labels: `format`, `reason`
    pub format_errors_total: IntCounterVec,
    /// Langfuse ingestion counts. Labels: `ingestion_type` (`trace`|`generation`)
    pub langfuse_ingestions: IntCounterVec,
    /// Rate limit hits. Labels: `model`, `scope` (`api_key`|`model`|`user`)
    pub token_rate_limit_hits_total: IntCounterVec,
    /// Prompt guard blocks. Labels: `reason`, `model`.
    pub prompt_guard_blocks_total: IntCounterVec,
    /// Semantic cache hits. Labels: `model`.
    pub cache_hits_total: IntCounterVec,
    /// Semantic cache misses. Labels: `model`.
    pub cache_misses_total: IntCounterVec,
    /// Model fallback events. Labels: `from_model`, `to_model`, `reason`
    pub fallback_total: IntCounterVec,
    /// Cumulative AI cost in micro-dollars (divide by 1_000_000 for dollars).
    /// Labels: `model`.
    pub cost_dollars_total: IntCounterVec,
    /// Per-request AI cost in micro-dollars (divide by 1_000_000 for dollars).
    /// Labels: `model`.
    pub cost_per_request_dollars: IntCounterVec,
    /// Content safety violations. Labels: `category`, `model`, `verdict`.
    pub content_safety_violations_total: IntCounterVec,
    /// PII entities detected/masked. Labels: `entity_type` (email, phone, etc.)
    pub pii_detected_total: IntCounterVec,
    /// Tenant access denied. Labels: `reason` (`unknown_key`|`quota_exceeded`|`model_not_allowed`).
    pub tenant_denied_total: IntCounterVec,
    /// A/B test variant selections. Labels: `experiment`, `variant`.
    pub ab_test_requests: IntCounterVec,

    // ── Plain IntCounters ────────────────────────────────────────
    /// Number of OpenTelemetry spans successfully exported.
    pub otel_spans_exported: IntCounter,
    /// Number of OpenTelemetry export errors.
    pub otel_export_errors: IntCounter,
    /// Number of Langfuse ingestion errors.
    pub langfuse_errors: IntCounter,

    // ── HistogramVecs ────────────────────────────────────────────
    /// Request duration (seconds). Labels: `model`, `provider`
    pub request_duration: HistogramVec,
    /// Time-to-first-token latency (seconds). Labels: `model`, `provider`
    pub first_token_latency: HistogramVec,
    /// Total tokens per request. Labels: `model`, `provider`
    pub tokens_per_request: HistogramVec,
}

// ── Histogram bucket definitions ──────────────────────────────────

const DURATION_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

const FIRST_TOKEN_BUCKETS: &[f64] = &[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0];

const TOKENS_PER_REQUEST_BUCKETS: &[f64] = &[10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0, 10000.0];

impl AIMetrics {
    /// Create and register all AI metrics with the given registry.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        Ok(Self {
            tokens_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_tokens_total",
                    "Total token counts broken down by model and direction (prompt/completion)."
                ),
                &["model", "direction"],
                registry
            )?,

            requests_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_requests_total",
                    "Total AI request counts broken down by model, format, and status."
                ),
                &["model", "format", "status"],
                registry
            )?,

            stream_events_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_stream_events_total",
                    "Total stream event counts broken down by model and event type."
                ),
                &["model", "event_type"],
                registry
            )?,

            backend_errors_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_backend_errors_total",
                    "Total backend (LLM provider) errors broken down by model and status code."
                ),
                &["model", "status_code"],
                registry
            )?,

            format_errors_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_format_errors_total",
                    "Total format-related errors broken down by format and reason."
                ),
                &["format", "reason"],
                registry
            )?,

            langfuse_ingestions: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_langfuse_ingestions_total",
                    "Total Langfuse ingestion counts broken down by ingestion type."
                ),
                &["ingestion_type"],
                registry
            )?,

            token_rate_limit_hits_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_token_rate_limit_hits_total",
                    "Total rate limit hits broken down by model and scope (api_key/model/user)."
                ),
                &["model", "scope"],
                registry
            )?,

            prompt_guard_blocks_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_prompt_guard_blocks_total",
                    "Total prompt guard blocks broken down by reason and model."
                ),
                &["reason", "model"],
                registry
            )?,

            cache_hits_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_cache_hits_total",
                    "Total semantic cache hits broken down by model."
                ),
                &["model"],
                registry
            )?,

            cache_misses_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_cache_misses_total",
                    "Total semantic cache misses broken down by model."
                ),
                &["model"],
                registry
            )?,

            fallback_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_fallback_total",
                    "Total model fallback invocations broken down by from/tomodel and reason."
                ),
                &["from_model", "to_model", "reason"],
                registry
            )?,

            cost_dollars_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_cost_dollars_total",
                    "Cumulative AI cost in micro-dollars (divide by 1_000_000 for dollars). Labels: model."
                ),
                &["model"],
                registry
            )?,

            cost_per_request_dollars: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_cost_per_request_dollars",
                    "Per-request AI cost in micro-dollars (divide by 1_000_000 for dollars). Labels: model."
                ),
                &["model"],
                registry
            )?,

            content_safety_violations_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_content_safety_violations_total",
                    "Total content safety violations broken down by category, model, and verdict."
                ),
                &["category", "model", "verdict"],
                registry
            )?,

            pii_detected_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_pii_detected_total",
                    "Total PII entities detected and masked broken down by entity type."
                ),
                &["entity_type"],
                registry
            )?,

            tenant_denied_total: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_tenant_denied_total",
                    "Total tenant access denials broken down by reason (unknown_key, quota_exceeded, model_not_allowed)."
                ),
                &["reason"],
                registry
            )?,

            ab_test_requests: register_int_counter_vec_with_registry!(
                Opts::new(
                    "nantian_gw_ai_ab_test_requests_total",
                    "Total A/B test variant selections broken down by experiment and variant."
                ),
                &["experiment", "variant"],
                registry
            )?,

            otel_spans_exported: register_int_counter_with_registry!(
                Opts::new(
                    "nantian_gw_ai_otel_spans_exported_total",
                    "Total number of OpenTelemetry spans successfully exported."
                ),
                registry
            )?,

            otel_export_errors: register_int_counter_with_registry!(
                Opts::new(
                    "nantian_gw_ai_otel_export_errors_total",
                    "Total number of OpenTelemetry export errors."
                ),
                registry
            )?,

            langfuse_errors: register_int_counter_with_registry!(
                Opts::new(
                    "nantian_gw_ai_langfuse_errors_total",
                    "Total number of Langfuse ingestion errors."
                ),
                registry
            )?,

            request_duration: register_histogram_vec_with_registry!(
                HistogramOpts::new(
                    "nantian_gw_ai_request_duration_seconds",
                    "AI request duration in seconds."
                )
                .buckets(DURATION_BUCKETS.to_vec()),
                &["model", "provider"],
                registry
            )?,

            first_token_latency: register_histogram_vec_with_registry!(
                HistogramOpts::new(
                    "nantian_gw_ai_first_token_latency_seconds",
                    "Time-to-first-token latency in seconds."
                )
                .buckets(FIRST_TOKEN_BUCKETS.to_vec()),
                &["model", "provider"],
                registry
            )?,

            tokens_per_request: register_histogram_vec_with_registry!(
                HistogramOpts::new("nantian_gw_ai_tokens_per_request", "Total tokens per request.")
                    .buckets(TOKENS_PER_REQUEST_BUCKETS.to_vec()),
                &["model", "provider"],
                registry
            )?,
        })
    }

    /// Record token usage for a single request.
    ///
    /// Increments the `tokens_total` counter for both `prompt` and `completion`
    /// directions independently.
    pub fn record_tokens(&self, model: &str, prompt_tokens: u64, completion_tokens: u64) {
        self.tokens_total
            .with_label_values(&[model, "prompt"])
            .inc_by(prompt_tokens);
        self.tokens_total
            .with_label_values(&[model, "completion"])
            .inc_by(completion_tokens);
    }

    /// Record a completed AI request.
    ///
    /// Increments `requests_total` for the given model, format, and status label,
    /// and observes the duration on the `request_duration` histogram.
    pub fn record_request(&self, model: &str, format: &str, status: &str, duration_secs: f64) {
        self.requests_total
            .with_label_values(&[model, format, status])
            .inc();
        self.request_duration
            .with_label_values(&[model, ""])
            .observe(duration_secs);
    }

    /// Record first-token latency for a request.
    pub fn record_first_token_latency(&self, model: &str, provider: &str, latency_secs: f64) {
        self.first_token_latency
            .with_label_values(&[model, provider])
            .observe(latency_secs);
    }

    /// Record tokens per request.
    pub fn record_tokens_per_request(&self, model: &str, provider: &str, tokens: f64) {
        self.tokens_per_request
            .with_label_values(&[model, provider])
            .observe(tokens);
    }

    /// Record a single stream event.
    ///
    /// `event_type` is typically `content`, `done`, or `error`.
    pub fn record_stream_event(&self, model: &str, event_type: &str) {
        self.stream_events_total
            .with_label_values(&[model, event_type])
            .inc();
    }

    /// Record a format-level error.
    ///
    /// `format` identifies the serialization format (e.g. `openai`, `anthropic`),
    /// and `reason` is a short label describing the failure.
    pub fn record_format_error(&self, format: &str, reason: &str) {
        self.format_errors_total
            .with_label_values(&[format, reason])
            .inc();
    }

    /// Record a backend (LLM provider) error.
    pub fn record_backend_error(&self, model: &str, status_code: &str) {
        self.backend_errors_total
            .with_label_values(&[model, status_code])
            .inc();
    }

    /// Record a rate limit hit.
    ///
    /// `scope` is typically `api_key`, `model`, or `user`.
    pub fn record_rate_limit_hit(&self, model: &str, scope: &str) {
        self.token_rate_limit_hits_total
            .with_label_values(&[model, scope])
            .inc();
    }

    /// Record a prompt guard block.
    pub fn record_prompt_guard_block(&self, reason: &str, model: &str) {
        self.prompt_guard_blocks_total
            .with_label_values(&[reason, model])
            .inc();
    }

    /// Record a model fallback event.
    pub fn record_fallback(&self, from_model: &str, to_model: &str, reason: &str) {
        self.fallback_total
            .with_label_values(&[from_model, to_model, reason])
            .inc();
    }

    /// Record a semantic cache hit.
    pub fn record_cache_hit(&self, model: &str) {
        self.cache_hits_total.with_label_values(&[model]).inc();
    }

    /// Record a semantic cache miss.
    pub fn record_cache_miss(&self, model: &str) {
        self.cache_misses_total.with_label_values(&[model]).inc();
    }

    /// Record AI cost in dollars. Stores micro-dollars internally.
    pub fn record_cost(&self, model: &str, dollars: f64) {
        let micro_dollars = (dollars * 1_000_000.0).max(0.0) as u64;
        self.cost_dollars_total
            .with_label_values(&[model])
            .inc_by(micro_dollars);
        self.cost_per_request_dollars
            .with_label_values(&[model])
            .inc_by(micro_dollars);
    }

    /// Record a content safety violation.
    pub fn record_content_safety_violation(&self, category: &str, model: &str, verdict: &str) {
        self.content_safety_violations_total
            .with_label_values(&[category, model, verdict])
            .inc();
    }

    /// Record PII entity detections. Increments per entity type label.
    pub fn record_pii_detected(&self, entity_type: &str, count: u64) {
        self.pii_detected_total
            .with_label_values(&[entity_type])
            .inc_by(count);
    }

    /// Record a tenant access denial.
    ///
    /// `reason` is typically `unknown_key`, `quota_exceeded`, or `model_not_allowed`.
    pub fn record_tenant_denied(&self, reason: &str) {
        self.tenant_denied_total.with_label_values(&[reason]).inc();
    }

    /// Record an A/B test variant selection.
    pub fn record_ab_test(&self, experiment: &str, variant: &str) {
        self.ab_test_requests
            .with_label_values(&[experiment, variant])
            .inc();
    }
}