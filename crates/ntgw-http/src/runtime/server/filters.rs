use super::*;
use std::{collections::HashSet, sync::Arc};

pub(super) fn build_ai_filter(
    snapshot: &SharedSnapshot,
    wasm_filter: Option<Arc<ntgw_ai::wasm_filter::WasmPluginFilter>>,
) -> Option<Arc<ntgw_ai::filter::AIGatewayFilter>> {
    use ntgw_ai::filter::AIGatewayFilterBuilder;
    use ntgw_ai::format::AdapterRegistry;
    use ntgw_ai::format::anthropic::AnthropicAdapter;
    use ntgw_ai::format::ollama::OllamaAdapter;
    use ntgw_ai::format::openai::OpenAIAdapter;
    use ntgw_ai::observability::metrics::AIMetrics;
    use ntgw_ai::ratelimit::{RateLimitConfig, TokenRateLimiter};

    let registry = prometheus::default_registry();
    let metrics = match AIMetrics::new(registry) {
        Ok(m) => Arc::new(m),
        Err(e) => {
            tracing::warn!(
                target: "ai_gateway",
                error = %e,
                "failed to create AI metrics, AI gateway disabled"
            );
            return None;
        }
    };

    let mut adapters = AdapterRegistry::new();
    adapters.register("openai", Arc::new(OpenAIAdapter));
    adapters.register("anthropic", Arc::new(AnthropicAdapter));
    adapters.register("ollama", Arc::new(OllamaAdapter));
    let adapters = Arc::new(adapters);

    let rate_limiter = {
        let snap = snapshot.load();
        snap.backends.iter().find_map(|b| {
            b.token_policy.as_ref().map(|tp| {
                TokenRateLimiter::new(RateLimitConfig {
                    tokens_per_minute: tp.tokens_per_minute,
                    tokens_per_hour: tp.tokens_per_hour,
                    requests_per_minute: tp.requests_per_minute,
                    scope: tp.scope.clone(),
                    burst: tp.burst,
                    on_limit: tp.on_limit.clone(),
                })
            })
        })
    };

    let mut builder = AIGatewayFilterBuilder::new(adapters, metrics);
    if let Some(rl) = rate_limiter {
        builder = builder.rate_limiter(rl);
    }
    if let Some(wf) = wasm_filter {
        builder = builder.wasm_filter(wf);
    }
    // Langfuse is wired via env vars. Other subsystems (cost_tracker, pii,
    // prompt_guard, content_safety, model_router, fallback, ab_engine,
    // tenant_manager, ai_sandbox, prompt_injector) are configured per-AIService
    // CRD through the xDS snapshot and applied at request time.
    if let Some(lf) = super::langfuse::build_langfuse_client() {
        builder = builder.langfuse(lf);
    }
    Some(Arc::new(builder.build()))
}

pub(super) fn build_wasm_filter(
    snapshot: &SharedSnapshot,
    max_concurrency: usize,
) -> Option<Arc<ntgw_ai::wasm_filter::WasmPluginFilter>> {
    use ntgw_ai::wasm_filter::WasmPluginFilter;
    use ntgw_wasm::plugin::{WasmHook, WasmPluginSpec, WasmSandboxConfig, global_plugin_manager};

    let snapshot_guard = snapshot.load();
    let mut desired: Vec<WasmPluginSpec> = Vec::new();

    for backend in &snapshot_guard.backends {
        let Some(ref wp) = backend.wasm_plugin else {
            continue;
        };
        if wp.wasm_bytes.is_empty() {
            continue;
        }

        let hooks: Vec<WasmHook> = wp
            .hooks
            .iter()
            .filter_map(|h| {
                serde_json::from_value(serde_json::Value::String(h.clone()))
                    .ok()
                    .or_else(|| {
                        tracing::warn!(
                            target: "wasm",
                            backend = %backend.name,
                            hook = %h,
                            "unknown wasm hook, skipping"
                        );
                        None
                    })
            })
            .collect();

        if hooks.is_empty() {
            tracing::warn!(
                target: "wasm",
                backend = %backend.name,
                "no valid hooks configured for wasm plugin, skipping"
            );
            continue;
        }

        let config: serde_json::Value =
            serde_json::from_str(&wp.config_json).unwrap_or(serde_json::Value::Null);

        let sandbox = WasmSandboxConfig {
            max_memory_bytes: {
                let mb = wp.sandbox.max_memory_bytes;
                if mb > usize::MAX as u64 {
                    usize::MAX
                } else {
                    mb as usize
                }
            },
            max_execution_ms: wp.sandbox.max_execution_time_ms,
            allow_network: wp.sandbox.allow_network,
            allow_file_system: wp.sandbox.allow_file_system,
        };

        desired.push((
            wp.name.clone(),
            wp.wasm_bytes.clone(),
            config,
            hooks,
            sandbox,
            if wp.sha256.is_empty() {
                None
            } else {
                Some(wp.sha256.clone())
            },
        ));
    }

    drop(snapshot_guard);

    let pm = match global_plugin_manager() {
        Ok(pm) => pm,
        Err(error) => {
            tracing::warn!(
                target: "wasm",
                error = %error,
                "failed to initialize wasm plugin manager"
            );
            return None;
        }
    };

    if desired.is_empty() {
        for name in pm.plugin_names() {
            pm.unload_plugin(&name);
        }
        return None;
    }

    let on_request_plugin_names = plugin_names_for_hook(&desired, WasmHook::OnRequest);
    let on_response_plugin_names = plugin_names_for_hook(&desired, WasmHook::OnResponse);

    let (loaded, updated, skipped, unloaded) = pm.diff_and_apply(&desired);
    tracing::info!(
        target: "wasm",
        loaded,
        updated,
        skipped,
        unloaded,
        "applied wasm plugin snapshot"
    );

    let plugin_names = pm.plugin_names();
    if plugin_names.is_empty() {
        return None;
    }

    Some(Arc::new(WasmPluginFilter::new_with_hook_plugins(
        pm,
        plugin_names,
        max_concurrency,
        on_request_plugin_names,
        on_response_plugin_names,
    )))
}

fn plugin_names_for_hook(
    desired: &[ntgw_wasm::plugin::WasmPluginSpec],
    hook: ntgw_wasm::plugin::WasmHook,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for (name, _, _, hooks, _, _) in desired {
        if hooks.contains(&hook) && seen.insert(name.as_str()) {
            names.push(name.clone());
        }
    }
    names
}
