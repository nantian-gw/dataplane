use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ntgw_wasm::sandbox::AISandbox;

use crate::ab_test::ABTestEngine;
use crate::content_safety::ContentSafetyFilter;
use crate::cost::CostTracker;
use crate::error::AIError;
use crate::fallback::ModelFallback;
use crate::format::ir::{AIRequest, AIStreamChunk};
use crate::format::{AdapterRegistry, detect_format};
use crate::keyring::ApiKeyManager;
use crate::model_router::{Complexity, ModelRouter};
use crate::multitenant::TenantManager;
use crate::observability::langfuse::LangfuseClient;
use crate::observability::metrics::AIMetrics;
use crate::observability::tracing::AITracer;
use crate::pii::PIIMasker;
use crate::prompt_guard::{PromptGuardFilter, message_text};
use crate::prompt_template::PromptInjector;
use crate::ratelimit::{RateLimitResult, TokenRateLimiter};
use crate::semantic_cache::{SemanticCache, build_cache_key};
use crate::token::TokenCounter;
use crate::wasm_filter::WasmPluginFilter;

/// Context stored between pre_process and post_process.
#[derive(Debug, Clone)]
pub struct AIContext {
    pub format: String,
    pub request: AIRequest,
    pub start_time: Instant,
    pub raw_request: Vec<u8>,
    pub cache_key: Option<String>,
    pub rate_limit_key: Option<String>,
    pub complexity: Option<Complexity>,
    /// OTel span tracking the upstream inference call. Created in
    /// process_with_fallback before the upstream call and ended in
    /// post_process so the span measures the actual upstream time.
    pub ai_span: std::sync::Arc<parking_lot::Mutex<Option<crate::observability::tracing::AISpan>>>,
}

/// AI Gateway filter: wraps upstream call with format conversion,
/// token counting, metrics, tracing, Langfuse.
/// Resolve the rate limit key based on the configured scope.
fn rate_limit_key(
    scope: &str,
    api_key: Option<&str>,
    model: &str,
    path: &str,
    user: &str,
) -> String {
    match scope {
        "apiKey" => api_key.unwrap_or("unknown").to_string(),
        "model" => model.to_string(),
        "user" if !user.is_empty() => user.to_string(),
        _ => path.to_string(), // "route" or default
    }
}

pub struct AIGatewayFilter {
    pub adapters: Arc<AdapterRegistry>,
    pub metrics: Arc<AIMetrics>,
    pub langfuse: Option<Arc<LangfuseClient>>,
    pub tracer: Option<Arc<AITracer>>,
    pub rate_limiter: Option<TokenRateLimiter>,
    pub key_manager: Option<Arc<ApiKeyManager>>,
    pub pii_masker: Option<PIIMasker>,
    pub prompt_guard: Option<PromptGuardFilter>,
    pub content_safety: Option<ContentSafetyFilter>,
    pub fallback: Option<ModelFallback>,
    pub cost_tracker: Option<Arc<CostTracker>>,
    pub model_router: Option<Arc<ModelRouter>>,
    pub prompt_injector: Option<Arc<PromptInjector>>,
    pub prompt_template_name: Option<String>,
    pub tenant_manager: Option<Arc<TenantManager>>,
    pub ab_engine: Option<Arc<ABTestEngine>>,
    pub ab_experiment_id: Option<String>,
    pub wasm_filter: Option<Arc<WasmPluginFilter>>,
    pub ai_sandbox: Option<Arc<AISandbox>>,
    pub semantic_cache: Option<Arc<SemanticCache>>,
}

/// Builder for `AIGatewayFilter`.
pub struct AIGatewayFilterBuilder {
    adapters: Arc<AdapterRegistry>,
    metrics: Arc<AIMetrics>,
    langfuse: Option<Arc<LangfuseClient>>,
    tracer: Option<Arc<AITracer>>,
    rate_limiter: Option<TokenRateLimiter>,
    key_manager: Option<Arc<ApiKeyManager>>,
    pii_masker: Option<PIIMasker>,
    prompt_guard: Option<PromptGuardFilter>,
    content_safety: Option<ContentSafetyFilter>,
    fallback: Option<ModelFallback>,
    cost_tracker: Option<Arc<CostTracker>>,
    model_router: Option<Arc<ModelRouter>>,
    prompt_injector: Option<Arc<PromptInjector>>,
    prompt_template_name: Option<String>,
    tenant_manager: Option<Arc<TenantManager>>,
    ab_engine: Option<Arc<ABTestEngine>>,
    ab_experiment_id: Option<String>,
    wasm_filter: Option<Arc<WasmPluginFilter>>,
    ai_sandbox: Option<Arc<AISandbox>>,
    semantic_cache: Option<Arc<SemanticCache>>,
}

impl AIGatewayFilterBuilder {
    pub fn new(adapters: Arc<AdapterRegistry>, metrics: Arc<AIMetrics>) -> Self {
        Self {
            adapters,
            metrics,
            langfuse: None,
            tracer: None,
            rate_limiter: None,
            key_manager: None,
            pii_masker: None,
            prompt_guard: None,
            content_safety: None,
            fallback: None,
            cost_tracker: None,
            model_router: None,
            prompt_injector: None,
            prompt_template_name: None,
            tenant_manager: None,
            ab_engine: None,
            ab_experiment_id: None,
            wasm_filter: None,
            ai_sandbox: None,
            semantic_cache: None,
        }
    }

    pub fn langfuse(mut self, v: Arc<LangfuseClient>) -> Self {
        self.langfuse = Some(v);
        self
    }
    pub fn tracer(mut self, v: Arc<AITracer>) -> Self {
        self.tracer = Some(v);
        self
    }
    pub fn rate_limiter(mut self, v: TokenRateLimiter) -> Self {
        self.rate_limiter = Some(v);
        self
    }
    pub fn key_manager(mut self, v: Arc<ApiKeyManager>) -> Self {
        self.key_manager = Some(v);
        self
    }
    pub fn pii_masker(mut self, v: PIIMasker) -> Self {
        self.pii_masker = Some(v);
        self
    }
    pub fn prompt_guard(mut self, v: PromptGuardFilter) -> Self {
        self.prompt_guard = Some(v);
        self
    }
    pub fn content_safety(mut self, v: ContentSafetyFilter) -> Self {
        self.content_safety = Some(v);
        self
    }
    pub fn fallback(mut self, v: ModelFallback) -> Self {
        self.fallback = Some(v);
        self
    }
    pub fn cost_tracker(mut self, v: Arc<CostTracker>) -> Self {
        self.cost_tracker = Some(v);
        self
    }
    pub fn model_router(mut self, v: Arc<ModelRouter>) -> Self {
        self.model_router = Some(v);
        self
    }
    pub fn prompt_injector(mut self, v: Arc<PromptInjector>) -> Self {
        self.prompt_injector = Some(v);
        self
    }
    pub fn prompt_template_name(mut self, v: String) -> Self {
        self.prompt_template_name = Some(v);
        self
    }
    pub fn tenant_manager(mut self, v: Arc<TenantManager>) -> Self {
        self.tenant_manager = Some(v);
        self
    }
    pub fn ab_engine(mut self, v: Arc<ABTestEngine>) -> Self {
        self.ab_engine = Some(v);
        self
    }
    pub fn ab_experiment_id(mut self, v: String) -> Self {
        self.ab_experiment_id = Some(v);
        self
    }
    pub fn wasm_filter(mut self, v: Arc<WasmPluginFilter>) -> Self {
        self.wasm_filter = Some(v);
        self
    }
    pub fn ai_sandbox(mut self, v: Arc<AISandbox>) -> Self {
        self.ai_sandbox = Some(v);
        self
    }
    pub fn semantic_cache(mut self, v: Arc<SemanticCache>) -> Self {
        self.semantic_cache = Some(v);
        self
    }

    pub fn build(self) -> AIGatewayFilter {
        AIGatewayFilter {
            adapters: self.adapters,
            metrics: self.metrics,
            langfuse: self.langfuse,
            tracer: self.tracer,
            rate_limiter: self.rate_limiter,
            key_manager: self.key_manager,
            pii_masker: self.pii_masker,
            prompt_guard: self.prompt_guard,
            content_safety: self.content_safety,
            fallback: self.fallback,
            cost_tracker: self.cost_tracker,
            model_router: self.model_router,
            prompt_injector: self.prompt_injector,
            prompt_template_name: self.prompt_template_name,
            tenant_manager: self.tenant_manager,
            ab_engine: self.ab_engine,
            ab_experiment_id: self.ab_experiment_id,
            wasm_filter: self.wasm_filter,
            ai_sandbox: self.ai_sandbox,
            semantic_cache: self.semantic_cache,
        }
    }
}

impl AIGatewayFilter {
    fn scoped_rate_limit_key(
        &self,
        path: &str,
        api_key: Option<&str>,
        request: &AIRequest,
    ) -> Option<String> {
        self.rate_limiter.as_ref().map(|rl| {
            rate_limit_key(
                &rl.config.scope,
                api_key,
                &request.model,
                path,
                request.user.as_deref().unwrap_or(""),
            )
        })
    }

    /// Single-pass security scan: combines prompt guard and content safety checks
    /// into one loop over request messages.
    fn scan_security(&self, request: &AIRequest) -> Result<(), AIError> {
        for msg in &request.messages {
            let text = match message_text(&msg.content) {
                Some(t) => t,
                None => continue,
            };

            self.check_prompt_guard(&text, &request.model)?;
            self.check_content_safety(&text, &request.model)?;
        }
        Ok(())
    }

    fn check_prompt_guard(&self, text: &str, model: &str) -> Result<(), AIError> {
        let guard = match &self.prompt_guard {
            Some(g) if g.enabled => g,
            _ => return Ok(()),
        };
        for pattern in &guard.patterns {
            if let Some(matched) = pattern.find(text) {
                let reason_str = "injection_pattern_match";
                let matched_str = matched.as_str();
                tracing::warn!(reason = %reason_str, matched = %matched_str, %model, "prompt guard blocked request");
                self.metrics.record_prompt_guard_block(reason_str, model);
                if guard.mode() == "block" {
                    return Err(AIError::PromptGuardBlocked {
                        reason: reason_str.to_string(),
                        matched: matched_str.to_string(),
                    });
                }
            }
        }
        let text_lower = text.to_lowercase();
        for keyword in &guard.keywords {
            if text_lower.contains(&keyword.to_lowercase()) {
                let reason = format!("blocked_keyword: {keyword}");
                tracing::warn!(reason = %reason, matched = %keyword, %model, "prompt guard blocked request");
                self.metrics.record_prompt_guard_block(&reason, model);
                if guard.mode() == "block" {
                    return Err(AIError::PromptGuardBlocked {
                        reason,
                        matched: keyword.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn check_content_safety(&self, text: &str, model: &str) -> Result<(), AIError> {
        let safety = match &self.content_safety {
            Some(s) if s.enabled => s,
            _ => return Ok(()),
        };
        for (category, regex) in &safety.patterns {
            if let Some(captured) = regex.find(text) {
                let matched_str = captured.as_str();
                if safety.block_mode {
                    tracing::warn!(category = %category, matched = %matched_str, %model, "content safety filter blocked request");
                    self.metrics
                        .record_content_safety_violation(category, model, "block");
                    return Err(AIError::ContentSafetyBlocked {
                        category: category.clone(),
                        matched: matched_str.to_string(),
                    });
                }
                tracing::warn!(category = %category, matched = %matched_str, %model, "content safety filter flagged request");
                self.metrics
                    .record_content_safety_violation(category, model, "flag");
            }
        }
        let text_lower_safety = text.to_lowercase();
        for (category, keyword) in &safety.keywords {
            if text_lower_safety.contains(&keyword.to_lowercase()) {
                if safety.block_mode {
                    self.metrics
                        .record_content_safety_violation(category, model, "block");
                    return Err(AIError::ContentSafetyBlocked {
                        category: category.clone(),
                        matched: keyword.clone(),
                    });
                }
                self.metrics
                    .record_content_safety_violation(category, model, "flag");
            }
        }
        Ok(())
    }

    pub async fn pre_process(
        &self,
        path: &str,
        body: &[u8],
        api_key: Option<&str>,
    ) -> Result<AIContext, AIError> {
        // 1. Detect format from path
        let fmt =
            detect_format(path).ok_or_else(|| AIError::UnsupportedFormat(path.to_string()))?;

        // 1b. Apply PII masking to raw body before parsing (privacy-safe)
        let masked_body = if let Some(ref masker) = self.pii_masker {
            let (masked, count, details) = masker.mask(std::str::from_utf8(body).unwrap_or(""));
            if count > 0 {
                let mut type_counts: std::collections::HashMap<String, u64> =
                    std::collections::HashMap::new();
                for (_, replacement) in &details {
                    // Extract entity type from replacement like "[email]" or "<phone>"
                    let entity_type = replacement
                        .trim_start_matches('[')
                        .trim_start_matches('<')
                        .trim_end_matches(']')
                        .trim_end_matches('>');
                    *type_counts.entry(entity_type.to_string()).or_insert(0) += 1;
                }
                for (entity_type, n) in &type_counts {
                    self.metrics.record_pii_detected(entity_type, *n);
                }
            }
            masked.into_owned().into_bytes()
        } else {
            body.to_vec()
        };

        // 2. Parse request body via adapter
        let adapter = self
            .adapters
            .get(fmt)
            .ok_or_else(|| AIError::AdapterNotFound(fmt.to_string()))?;
        let mut request = adapter.parse_request(&masked_body)?;

        // 2a. Wasm plugin pre-processing (before rate limiting, after format detection)
        if let Some(ref wf) = self.wasm_filter {
            let mut headers = HashMap::new();
            if let Some(ref key) = api_key {
                headers.insert("x-api-key".to_string(), key.to_string());
            }
            headers.insert("x-request-model".to_string(), request.model.clone());
            wf.pre_process(headers, masked_body.clone())
                .await
                .map_err(|e| AIError::Internal(anyhow::anyhow!("wasm plugin rejected: {e}")))?;
        }

        // 2b. Model routing: classify and replace model if router configured.
        let mut complexity: Option<Complexity> = None;
        if let Some(ref router) = self.model_router {
            let complexity_val = router.classify(&request);
            complexity = Some(complexity_val);
            if let Some(route) = router.route(complexity_val) {
                tracing::info!(
                    original_model = %request.model,
                    routed_model = %route.model,
                    ?complexity,
                    "model routed"
                );
                request.model = route.model.clone();
                if let Some(max_tokens) = route.max_tokens
                    && request
                        .max_tokens
                        .is_none_or(|req_max| req_max > max_tokens)
                {
                    request.max_tokens = Some(max_tokens);
                }
            }
        }

        // 2c. A/B test experiment: override model if a variant is selected.
        if let Some(ref engine) = self.ab_engine
            && let Some(ref experiment_id) = self.ab_experiment_id
            && let Some(variant) = engine.select_variant(experiment_id)
        {
            tracing::info!(
                original_model = %request.model,
                variant = %variant.name,
                experiment = %experiment_id,
                ab_model = %variant.model,
                "A/B test variant selected"
            );
            self.metrics.record_ab_test(experiment_id, &variant.name);
            request.model = variant.model;
        }

        // 2d. Prompt template injection.
        if let Some(ref injector) = self.prompt_injector {
            let name = self.prompt_template_name.as_deref().unwrap_or("default");
            if injector.inject(name, &mut request) {
                tracing::info!(template = %name, "prompt template injected");
            }
        }

        // 2e+2f. Merged security scan: prompt guard + content safety in one message loop.
        if self.prompt_guard.is_some() || self.content_safety.is_some() {
            self.scan_security(&request)?;
        }

        // 3. Rate limit check (pre-request)
        let rate_limit_key = self.scoped_rate_limit_key(path, api_key, &request);
        if let (Some(rl), Some(rl_key)) = (&self.rate_limiter, &rate_limit_key) {
            match rl.check(rl_key) {
                RateLimitResult::Limited { retry_after_secs } => {
                    self.metrics
                        .record_rate_limit_hit(&request.model, &rl.config.scope);
                    match rl.config.on_limit.as_str() {
                        "queue" => {
                            // Queue mode: sleep and retry once
                            tokio::time::sleep(Duration::from_secs(retry_after_secs.min(5))).await;
                            // Re-check after waiting
                            match rl.check(rl_key) {
                                RateLimitResult::Limited { .. } => {
                                    return Err(AIError::RateLimitExceeded { retry_after_secs });
                                }
                                RateLimitResult::Allowed { .. } => {}
                            }
                        }
                        "warn" => {
                            // Warn mode: log and allow
                            tracing::warn!(
                                target: "rate_limiter",
                                scope = %rl.config.scope,
                                key = %rl_key,
                                "rate limit exceeded (warn mode, allowing)"
                            );
                        }
                        _ => {
                            // Default: reject
                            return Err(AIError::RateLimitExceeded { retry_after_secs });
                        }
                    }
                }
                RateLimitResult::Allowed { .. } => {}
            }
        }

        // 3b. Tenant checks: resolve, quota, model access
        if let (Some(api_key), Some(tm)) = (api_key, &self.tenant_manager) {
            let tenant = tm.resolve(api_key).ok_or_else(|| {
                self.metrics.record_tenant_denied("unknown_key");
                AIError::TenantDenied("unknown api key".into())
            })?;

            let estimated_tokens = request.max_tokens.unwrap_or(1) as u64;
            if !tm.check_quota(&tenant.tenant_id, estimated_tokens) {
                self.metrics.record_tenant_denied("quota_exceeded");
                return Err(AIError::TenantDenied("quota exceeded".into()));
            }

            if !tm.check_model_access(&tenant.tenant_id, &request.model) {
                self.metrics.record_tenant_denied("model_not_allowed");
                return Err(AIError::TenantDenied("model not allowed".into()));
            }
        }

        // 4. Semantic cache lookup
        let cache_key = if let Some(ref cache) = self.semantic_cache {
            if let Some(cached_response) = cache.lookup(&request) {
                self.metrics.record_cache_hit(&request.model);
                return Err(AIError::CacheHit {
                    response: Box::new(cached_response),
                });
            }
            self.metrics.record_cache_miss(&request.model);
            Some(build_cache_key(&request))
        } else {
            None
        };

        Ok(AIContext {
            format: fmt.to_string(),
            request,
            start_time: Instant::now(),
            raw_request: masked_body,
            cache_key,
            rate_limit_key,
            complexity,
            ai_span: std::sync::Arc::new(parking_lot::Mutex::new(None)),
        })
    }

    /// Resolve a gateway API key to a backend credential via the configured
    /// `ApiKeyManager`. Returns `None` if no key manager is configured or if
    /// no matching credential exists.
    #[must_use]
    pub fn resolve_api_key(&self, gateway_key: &str) -> Option<crate::keyring::BackendCredential> {
        self.key_manager
            .as_ref()
            .and_then(|km| km.resolve(gateway_key))
    }

    /// Post-upstream: format response, count tokens, emit metrics,
    /// ingest to Langfuse, optionally end OTel span.
    ///
    /// Returns the reformatted response body.
    pub async fn post_process(
        &self,
        ctx: AIContext,
        response_body: &[u8],
        response_status: u16,
    ) -> Result<Vec<u8>, AIError> {
        let adapter = self
            .adapters
            .get(&ctx.format)
            .ok_or_else(|| AIError::AdapterNotFound(ctx.format.clone()))?;
        let duration = ctx.start_time.elapsed();
        let is_stream = ctx.request.stream;
        let model = ctx.request.model.clone();
        let format = ctx.format.clone();

        // Wasm plugin on_response hook
        if let Some(ref wf) = self.wasm_filter {
            wf.post_process(HashMap::new(), response_body.to_vec())
                .await
                .map_err(|e| {
                    AIError::Internal(anyhow::anyhow!("wasm plugin response rejected: {e}"))
                })?;
        }

        let (usage, output_body) = if is_stream {
            // Parse SSE chunks, accumulate usage, reformat
            let mut counter = if let Some(ref sandbox) = self.ai_sandbox {
                TokenCounter::with_sandbox(Arc::clone(sandbox))
            } else {
                TokenCounter::new()
            };
            let sse_text = std::str::from_utf8(response_body).map_err(|e| {
                AIError::Internal(anyhow::anyhow!("SSE body is not valid UTF-8: {e}"))
            })?;
            let chunks: Vec<AIStreamChunk> = parse_sse_chunks(sse_text)?;

            for chunk in &chunks {
                counter.record_stream_chunk(chunk);
            }

            // Re-serialize each chunk through the adapter
            let mut reformatted = Vec::new();
            for chunk in &chunks {
                let serialized = adapter.serialize_stream_chunk(chunk)?;
                reformatted.extend_from_slice(serialized.as_bytes());
            }

            let usage = counter.accumulated_usage();

            // Record stream events metric
            let _event_count = chunks.len() as u64;
            self.metrics.record_stream_event(&model, "content");

            (Some(usage), reformatted)
        } else {
            // Parse as AIResponse
            let response = adapter.parse_response(response_body)?;
            let usage = response.usage.clone();

            if let (Some(cache), Some(cache_key)) = (&self.semantic_cache, &ctx.cache_key) {
                cache.store(cache_key, &response);
            }

            let serialized = adapter.serialize_response(&response)?;
            (usage, serialized)
        };

        // Record metrics
        if let Some(ref usage) = usage {
            self.metrics
                .record_tokens(&model, usage.prompt_tokens, usage.completion_tokens);

            // Record cost
            if let Some(ref tracker) = self.cost_tracker {
                let cost = tracker.record(&model, usage);
                self.metrics.record_cost(&model, cost);
            }

            // Record first-token latency and tokens per request
            self.metrics
                .record_first_token_latency(&model, "", duration.as_secs_f64());
            self.metrics
                .record_tokens_per_request(&model, "", usage.total_tokens as f64);
        }

        // Record rate limit token usage (post-response)
        if let (Some(rl), Some(rl_key)) = (&self.rate_limiter, &ctx.rate_limit_key) {
            let total_tokens = usage.as_ref().map(|u| u.total_tokens).unwrap_or(0);
            if let RateLimitResult::Limited { .. } = rl.record_usage(rl_key, total_tokens) {
                self.metrics.record_rate_limit_hit(&model, &rl.config.scope);
            }
        }

        let status_str = if response_status < 400 {
            "success"
        } else {
            "error"
        };
        self.metrics
            .record_request(&model, &format, status_str, duration.as_secs_f64());

        // Record format error and backend error if applicable (error status)
        if response_status >= 400 {
            self.metrics
                .record_format_error(&format, &format!("http_{response_status}"));
            self.metrics
                .record_backend_error(&model, &response_status.to_string());
        }

        // Langfuse ingestion
        if let Some(ref lf) = self.langfuse {
            let trace_id = uuid::Uuid::new_v4().to_string();
            let input_json: serde_json::Value =
                serde_json::from_slice(&ctx.raw_request).unwrap_or(serde_json::Value::Null);
            let output_json: serde_json::Value =
                serde_json::from_slice(response_body).unwrap_or(serde_json::Value::Null);

            if let Err(e) = lf
                .ingest_trace(
                    &trace_id,
                    ctx.request.user.as_deref(),
                    None,
                    &Default::default(),
                )
                .await
            {
                tracing::warn!(error = %e, "failed to ingest trace to Langfuse");
            }

            if let Some(ref usage) = usage
                && let Err(e) = lf
                    .ingest_generation(
                        &trace_id,
                        &model,
                        usage.prompt_tokens,
                        usage.completion_tokens,
                        duration.as_millis() as u64,
                        &input_json,
                        &output_json,
                        &Default::default(),
                    )
                    .await
            {
                tracing::warn!(error = %e, "failed to ingest generation to Langfuse");
            }
        }

        // OTel tracing — span was started in process_with_fallback
        if let (Some(tracer), Some(span)) = (self.tracer.as_ref(), ctx.ai_span.lock().take()) {
            let prompt_tokens = usage.as_ref().map_or(0, |u| u.prompt_tokens);
            let completion_tokens = usage.as_ref().map_or(0, |u| u.completion_tokens);
            tracer.end_span(span, prompt_tokens, completion_tokens);
        }

        // Mask PII in the response body before returning to client
        let output_body = if let Some(ref masker) = self.pii_masker {
            let (masked_response, count, details) =
                masker.mask(std::str::from_utf8(&output_body).unwrap_or(""));
            if count > 0 {
                let mut type_counts: std::collections::HashMap<String, u64> =
                    std::collections::HashMap::new();
                for (_, replacement) in &details {
                    let entity_type = replacement
                        .trim_start_matches('[')
                        .trim_start_matches('<')
                        .trim_end_matches(']')
                        .trim_end_matches('>');
                    *type_counts.entry(entity_type.to_string()).or_insert(0) += 1;
                }
                for (entity_type, n) in &type_counts {
                    self.metrics.record_pii_detected(entity_type, *n);
                }
            }
            masked_response.into_owned().into_bytes()
        } else {
            output_body
        };

        Ok(output_body)
    }

    /// Resolve the next fallback model in the chain. Returns `Some(&str)` if
    /// a fallback should be attempted, or `None` if the chain is exhausted or
    /// the current status/timeout does not trigger any fallback.
    pub fn resolve_fallback(
        &self,
        model: &str,
        status_code: u16,
        is_timeout: bool,
        attempt: u32,
    ) -> Option<&str> {
        self.fallback
            .as_ref()?
            .resolve_fallback(model, Some(status_code), is_timeout, attempt)
    }

    /// Process an AI request end-to-end with automatic model fallback.
    ///
    /// On upstream failures (non-2xx status or timeout), this method iterates
    /// through the fallback chain and retries with the next model. The caller
    /// provides `upstream_call`, which is invoked for each attempt with the
    /// (possibly modified) [`AIRequest`] and the raw request body.
    pub async fn process_with_fallback<F, Fut>(
        &self,
        path: &str,
        body: &[u8],
        api_key: Option<&str>,
        upstream_call: F,
    ) -> Result<Vec<u8>, AIError>
    where
        F: Fn(&AIRequest, &[u8]) -> Fut,
        Fut: std::future::Future<Output = Result<(Vec<u8>, u16), AIError>>,
    {
        let mut ctx = self.pre_process(path, body, api_key).await?;
        let original_model = ctx.request.model.clone();
        let mut attempt: u32 = 0;
        loop {
            // Start OTel span before upstream call to measure actual inference time.
            if let Some(ref tracer) = self.tracer {
                *ctx.ai_span.lock() = Some(tracer.start_span(
                    "ai.inference",
                    &ctx.request.model,
                    &ctx.format,
                    ctx.request.stream,
                ));
            }

            let (response_body, response_status) =
                upstream_call(&ctx.request, &ctx.raw_request).await?;

            let result = self
                .post_process(ctx, &response_body, response_status)
                .await;

            // If post_process succeeds or the error is not a backend-type error,
            // return the result immediately (no fallback for non-backend errors).
            if result.is_ok() || !self.should_attempt_fallback(response_status) {
                return result;
            }

            let is_timeout = response_status == 504 || response_status == 408;

            let next = self.fallback.as_ref().and_then(|fb| {
                fb.resolve_fallback(&original_model, Some(response_status), is_timeout, attempt)
            });

            let next_model = match next {
                Some(m) => m,
                None => {
                    return Err(AIError::FallbackExhausted {
                        model: original_model,
                        reason: format!(
                            "all fallbacks exhausted after {} attempt(s), last status={}",
                            attempt, response_status
                        ),
                    });
                }
            };

            self.metrics.record_fallback(
                &original_model,
                next_model,
                &format!("status_{response_status}"),
            );
            tracing::debug!(
                from = %original_model,
                to = %next_model,
                attempt,
                status = response_status,
                "model fallback triggered"
            );

            // Rebuild context for the next attempt with fallback model
            let mut new_ctx = self.pre_process(path, body, api_key).await?;
            new_ctx.request.model = next_model.to_string();
            new_ctx.rate_limit_key = self.scoped_rate_limit_key(path, api_key, &new_ctx.request);
            ctx = new_ctx;
            attempt += 1;
        }
    }

    /// Check whether a status code warrants attempting a model fallback.
    fn should_attempt_fallback(&self, status: u16) -> bool {
        status >= 500 || status == 429
    }
}

/// Parse SSE text into `AIStreamChunk`s. SSE events delimited by double
/// newline. Each event line starts with `data: `. Skip `[DONE]` lines.
pub fn parse_sse_chunks(sse_text: &str) -> Result<Vec<AIStreamChunk>, AIError> {
    let mut chunks = Vec::new();
    for event in sse_text.split("\n\n") {
        let event = event.trim();
        if event.is_empty() {
            continue;
        }
        for line in event.lines() {
            if let Some(json) = line.strip_prefix("data: ") {
                if json == "[DONE]" {
                    continue;
                }
                let chunk: AIStreamChunk = serde_json::from_str(json)
                    .map_err(|e| AIError::Internal(anyhow::anyhow!("SSE parse error: {e}")))?;
                chunks.push(chunk);
            }
        }
    }
    Ok(chunks)
}
