# Runtime Unwrap Governance First Batch

## Scope

This record covers only the 2026-06-15 first batch of runtime unwrap governance
for the `dataplane` repository. It is intentionally not a whole-workspace
inventory. The batch governs the four `ntgw-ai` production files sampled during
design review and explicitly documents the deferred invariant and test-only
examples that remain out of scope for this pass.

## Governed Call Sites

| File | Call Site | Bucket | First-Batch Action |
| --- | --- | --- | --- |
| `crates/ntgw-ai/src/observability/langfuse.rs` | `HeaderValue::from_str(...).expect(...)` | config | Replace with `LangfuseClient::new(...) -> Result<Self, AIError>` and return `AIError::Observability(...)` on invalid configuration |
| `crates/ntgw-ai/src/observability/langfuse.rs` | `reqwest::Client::builder().default_headers(...).build().expect(...)` | config | Replace with explicit `AIError::Observability(...)` propagation |
| `crates/ntgw-ai/src/format/openai.rs` | `serde_json::to_value(parts).expect("MultiPart serialization should not fail")` | runtime | Replace with a fallible OpenAI message-conversion helper that returns `AIError::FormatSerialize` |
| `crates/ntgw-ai/src/format/openai.rs` | `serde_json::to_vec(&error).unwrap()` | runtime | Return the structured body when serialization succeeds, otherwise return a static OpenAI fallback error payload |
| `crates/ntgw-ai/src/format/ollama.rs` | `serde_json::to_vec(&error).unwrap()` | runtime | Return the structured body when serialization succeeds, otherwise return a static Ollama fallback error payload |
| `crates/ntgw-ai/src/format/anthropic.rs` | `texts.into_iter().next().expect("non-empty texts")` | runtime | Replace with slice matching in `blocks_to_content()` |
| `crates/ntgw-ai/src/format/anthropic.rs` | `serde_json::to_string(&stop).unwrap()` | runtime | Replace with `AIError::FormatSerialize` propagation from a helper that renders the `message_stop` event |
| `crates/ntgw-ai/src/format/anthropic.rs` | `serde_json::to_vec(&error).unwrap()` | runtime | Return the structured body when serialization succeeds, otherwise return a static Anthropic fallback error payload |

## Deferred Samples

| File | Call Site | Bucket | Deferred Reason |
| --- | --- | --- | --- |
| `crates/ntgw-ai/src/filter.rs` | `self.adapters.expect("adapters required")` and `self.metrics.expect("metrics required")` | invariant | Builder-required invariants were explicitly approved for deferral in batch 1 |
| `crates/ntgw-ai/src/content_safety.rs` | `LazyLock` default regex `.expect(...)` calls | invariant | Fixed regex literals compiled at startup; not driven by untrusted runtime input |
| `crates/ntgw-ai/src/prompt_guard.rs` | `LazyLock` default regex `.expect(...)` calls | invariant | Fixed regex literals compiled at startup; not driven by untrusted runtime input |
| `crates/ntgw-ai/src/pii.rs` | default regex `.expect(...)` calls | invariant | Fixed regex literals compiled from checked-in patterns; not driven by external input |
| `crates/ntgw-stream/src/pool.rs` | inline async test `.unwrap()` calls | test | Test harness only |
| `crates/ntgw-http/src/proxy/logging.rs` | inline test/assertion `.expect()` calls | test | Test code only |
