use ntgw_ai::error::AIError;
use ntgw_ai::format::FormatAdapter;
use ntgw_ai::format::anthropic::AnthropicAdapter;
use ntgw_ai::format::ir::*;

fn load_fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/fixtures/anthropic/{}", name);
    std::fs::read(path).unwrap()
}

#[test]
fn test_parse_with_system() {
    let body = load_fixture("messages_request.json");
    let adapter = AnthropicAdapter;
    let req = adapter.parse_request(&body).unwrap();

    assert_eq!(req.model, "claude-sonnet-4-20250514");
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].role, AIRole::System);
    assert!(matches!(req.messages[0].content, AIContent::Text(_)));
    assert_eq!(req.messages[1].role, AIRole::User);
    assert_eq!(req.max_tokens, Some(1024));
}

#[test]
fn test_parse_no_system() {
    let body = load_fixture("messages_request_no_system.json");
    let adapter = AnthropicAdapter;
    let req = adapter.parse_request(&body).unwrap();

    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, AIRole::User);
    assert_eq!(req.max_tokens, Some(512));
}

#[test]
fn test_serialize_response() {
    let ai_resp = AIResponse {
        id: "msg_123".into(),
        model: "claude-sonnet-4-20250514".into(),
        choices: vec![AIChoice {
            index: 0,
            message: AIMessage {
                role: AIRole::Assistant,
                content: AIContent::Text("The capital of France is Paris.".into()),
                name: None,
                tool_calls: vec![],
                tool_call_id: None,
            },
            finish_reason: Some("end_turn".into()),
        }],
        usage: Some(AIUsage {
            prompt_tokens: 20,
            completion_tokens: 8,
            total_tokens: 28,
        }),
        created: None,
        extra: Default::default(),
    };

    let adapter = AnthropicAdapter;
    let body = adapter.serialize_response(&ai_resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(parsed["type"], "message");
    assert_eq!(parsed["role"], "assistant");
    assert_eq!(
        parsed["content"][0]["text"],
        "The capital of France is Paris."
    );
    assert_eq!(parsed["usage"]["input_tokens"], 20);
    assert_eq!(parsed["usage"]["output_tokens"], 8);
}

#[test]
fn test_serialize_stream_chunk() {
    let chunk = AIStreamChunk {
        id: "msg_123".into(),
        model: "claude-sonnet-4-20250514".into(),
        choices: vec![AIStreamChoice {
            index: 0,
            delta: AIStreamDelta {
                role: None,
                content: Some("Hello".into()),
                tool_calls: vec![],
            },
            finish_reason: None,
        }],
        usage: None,
        created: None,
    };

    let adapter = AnthropicAdapter;
    let sse = adapter.serialize_stream_chunk(&chunk).unwrap();

    assert!(sse.contains("event: content_block_delta"));
    assert!(sse.contains("data: "));
    assert!(sse.contains("Hello"));
}

#[test]
fn test_stream_final_chunk() {
    let chunk = AIStreamChunk {
        id: "msg_123".into(),
        model: "claude-sonnet-4-20250514".into(),
        choices: vec![AIStreamChoice {
            index: 0,
            delta: AIStreamDelta {
                role: None,
                content: None,
                tool_calls: vec![],
            },
            finish_reason: Some("end_turn".into()),
        }],
        usage: None,
        created: None,
    };

    let adapter = AnthropicAdapter;
    let sse = adapter.serialize_stream_chunk(&chunk).unwrap();

    assert!(sse.contains("event: message_stop"));
}

#[test]
fn test_parse_stream_body_reads_anthropic_events() {
    let adapter = AnthropicAdapter;
    let body = br#"event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":10,"output_tokens":2}}

event: message_stop
data: {"type":"message_stop"}

"#;

    let chunks = adapter
        .parse_stream_body(body)
        .expect("anthropic stream body should parse");

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].choices[0].delta.content.as_deref(), Some("Hello"));
    assert_eq!(
        chunks[1].choices[0].finish_reason.as_deref(),
        Some("end_turn")
    );
    assert_eq!(
        chunks[1].usage.as_ref().map(|usage| usage.total_tokens),
        Some(12)
    );
    assert_eq!(chunks[2].choices[0].finish_reason.as_deref(), Some("stop"));
}

#[test]
fn test_error_response() {
    let adapter = AnthropicAdapter;
    let body = adapter.error_response(429, "Too many requests").unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(parsed["type"], "error");
    assert_eq!(parsed["error"]["type"], "invalid_request_error");
    assert_eq!(parsed["error"]["message"], "Too many requests");
}

#[test]
fn test_parse_invalid_json() {
    let adapter = AnthropicAdapter;
    let result = adapter.parse_request(b"not json");
    assert!(matches!(result, Err(AIError::FormatParse { .. })));
}
