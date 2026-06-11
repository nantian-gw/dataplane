use std::sync::Arc;

use ntgw_ai::filter::{AIGatewayFilter, AIGatewayFilterBuilder, parse_sse_chunks};
use ntgw_ai::format::AdapterRegistry;
use ntgw_ai::format::anthropic::AnthropicAdapter;
use ntgw_ai::format::openai::OpenAIAdapter;
use ntgw_ai::observability::metrics::AIMetrics;
use prometheus::Registry;

fn test_filter() -> (AIGatewayFilter, Registry) {
    let registry = Registry::new();
    let metrics = AIMetrics::new(&registry).unwrap();
    let mut adapters = AdapterRegistry::new();
    adapters.register("openai", Arc::new(OpenAIAdapter));
    adapters.register("anthropic", Arc::new(AnthropicAdapter));
    let filter = AIGatewayFilterBuilder::new(Arc::new(adapters), Arc::new(metrics)).build();
    (filter, registry)
}

fn gather_metric_value(registry: &Registry, name: &str, labels: &[(&str, &str)]) -> f64 {
    let families = registry.gather();
    for family in &families {
        if family.get_name() == name {
            for m in family.get_metric() {
                if labels_match(m.get_label(), labels) {
                    if family.get_field_type() == prometheus::proto::MetricType::COUNTER {
                        return m.get_counter().get_value();
                    }
                }
            }
        }
    }
    0.0
}

fn labels_match(got: &[prometheus::proto::LabelPair], want: &[(&str, &str)]) -> bool {
    if got.len() != want.len() {
        return false;
    }
    for w in want {
        if !got
            .iter()
            .any(|lp| lp.get_name() == w.0 && lp.get_value() == w.1)
        {
            return false;
        }
    }
    true
}

#[tokio::test]
async fn test_openai_non_stream_roundtrip() {
    let (filter, _registry) = test_filter();

    let request_body =
        br#"{"model": "gpt-4o", "messages": [{"role": "user", "content": "hello"}]}"#;

    let ctx = filter
        .pre_process("/v1/chat/completions", request_body, None)
        .await
        .expect("pre_process should succeed");

    assert_eq!(ctx.format, "openai");
    assert_eq!(ctx.request.model, "gpt-4o");
    assert!(!ctx.request.stream);

    let response_body = br#"{
        "id": "chatcmpl-123",
        "object": "chat.completion",
        "model": "gpt-4o",
        "created": 1700000000,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hello!"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 2, "total_tokens": 12}
    }"#;

    let output = filter
        .post_process(ctx, response_body, 200)
        .await
        .expect("post_process should succeed");

    let parsed: serde_json::Value =
        serde_json::from_slice(&output).expect("output should be valid JSON");
    assert_eq!(parsed["object"], "chat.completion");
    assert_eq!(parsed["model"], "gpt-4o");
    assert_eq!(parsed["choices"][0]["message"]["role"], "assistant");
    assert_eq!(parsed["choices"][0]["message"]["content"], "Hello!");
}

#[tokio::test]
async fn test_anthropic_request_detection() {
    let (filter, _registry) = test_filter();

    let request_body =
        br#"{"model": "claude-3", "max_tokens": 100, "messages": [{"role": "user", "content": "hello"}]}"#;

    let ctx = filter
        .pre_process("/v1/messages", request_body, None)
        .await
        .expect("pre_process should succeed for anthropic path");

    assert_eq!(ctx.format, "anthropic");
    assert_eq!(ctx.request.model, "claude-3");
}

#[tokio::test]
async fn test_streaming_aggregation() {
    let (filter, _registry) = test_filter();

    let request_body =
        br#"{"model": "gpt-4o", "messages": [{"role": "user", "content": "hello"}], "stream": true}"#;

    let ctx = filter
        .pre_process("/v1/chat/completions", request_body, None)
        .await
        .expect("pre_process should succeed");

    assert!(ctx.request.stream);

    let sse_body = b"data: {\"id\":\"1\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\ndata: {\"id\":\"2\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world\"}}]}\n\ndata: {\"id\":\"3\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}\n\n";

    let output = filter
        .post_process(ctx, sse_body, 200)
        .await
        .expect("post_process for streaming should succeed");

    assert!(!output.is_empty());
    let output_str = String::from_utf8_lossy(&output);
    assert!(output_str.contains("data: "));
    assert!(output_str.contains("Hello"));
}

#[tokio::test]
async fn test_metrics_emission() {
    let (filter, registry) = test_filter();

    let request_body = br#"{"model": "gpt-4o", "messages": [{"role": "user", "content": "hi"}]}"#;

    let ctx = filter
        .pre_process("/v1/chat/completions", request_body, None)
        .await
        .expect("pre_process should succeed");

    let response_body = br#"{
        "id": "chatcmpl-456",
        "object": "chat.completion",
        "model": "gpt-4o",
        "created": 1700000000,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hi there!"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 8, "completion_tokens": 3, "total_tokens": 11}
    }"#;

    let _ = filter
        .post_process(ctx, response_body, 200)
        .await
        .expect("post_process should succeed");

    let request_count = gather_metric_value(
        &registry,
        "ai_requests_total",
        &[
            ("model", "gpt-4o"),
            ("format", "openai"),
            ("status", "success"),
        ],
    );
    assert!(
        request_count >= 1.0,
        "request count should be >= 1, got {}",
        request_count
    );

    let prompt_tokens = gather_metric_value(
        &registry,
        "ai_tokens_total",
        &[("model", "gpt-4o"), ("direction", "prompt")],
    );
    assert!(
        prompt_tokens >= 8.0,
        "prompt tokens should be >= 8, got {}",
        prompt_tokens
    );
}

#[tokio::test]
async fn test_noop_observability() {
    let registry = Registry::new();
    let metrics = AIMetrics::new(&registry).unwrap();
    let mut adapters = AdapterRegistry::new();
    adapters.register("openai", Arc::new(OpenAIAdapter));
    let filter = AIGatewayFilterBuilder::new(Arc::new(adapters), Arc::new(metrics)).build();

    let request_body = br#"{"model": "gpt-4o", "messages": [{"role": "user", "content": "test"}]}"#;
    let ctx = filter
        .pre_process("/v1/chat/completions", request_body, None)
        .await
        .expect("pre_process with noop observability should succeed");

    let response_body = br#"{
        "id": "chatcmpl-noop",
        "object": "chat.completion",
        "model": "gpt-4o",
        "created": 1700000000,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "ok"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    }"#;

    let output = filter
        .post_process(ctx, response_body, 200)
        .await
        .expect("post_process with noop observability should succeed");

    let parsed: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(parsed["choices"][0]["message"]["content"], "ok");
}

#[test]
fn test_parse_sse_chunks_basic() {
    let sse = r#"data: {"id":"1","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Hello"}}]}

data: {"id":"2","model":"gpt-4o","choices":[{"index":0,"delta":{"content":" world"}}]}

data: [DONE]
"#;

    let chunks = parse_sse_chunks(sse).expect("parse_sse_chunks should succeed");
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].id, "1");
    assert_eq!(chunks[1].id, "2");
}

#[test]
fn test_parse_sse_chunks_empty() {
    let chunks = parse_sse_chunks("").expect("empty SSE should return Ok");
    assert!(chunks.is_empty());
}

#[test]
fn test_parse_sse_chunks_with_usage() {
    let sse = r#"data: {"id":"1","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"Hello"}}],"usage":{"prompt_tokens":5,"completion_tokens":1,"total_tokens":6}}

data: [DONE]
"#;

    let chunks = parse_sse_chunks(sse).expect("parse_sse_chunks should succeed");
    assert_eq!(chunks.len(), 1);
    let usage = chunks[0].usage.as_ref().expect("should have usage");
    assert_eq!(usage.prompt_tokens, 5);
    assert_eq!(usage.completion_tokens, 1);
    assert_eq!(usage.total_tokens, 6);
}

#[test]
fn test_parse_sse_chunks_only_done() {
    let sse = "data: [DONE]\n\n";
    let chunks = parse_sse_chunks(sse).expect("only DONE should return Ok");
    assert!(chunks.is_empty());
}

#[test]
fn test_parse_sse_chunks_whitespace() {
    let sse = "\n\ndata: {\"id\":\"1\",\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"x\"}}]}\n\n\n";
    let chunks = parse_sse_chunks(sse).expect("whitespace handling should succeed");
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].id, "1");
}
