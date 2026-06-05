use aeg_ai::error::AIError;
use aeg_ai::format::ir::*;
use aeg_ai::format::openai::OpenAIAdapter;
use aeg_ai::format::FormatAdapter;

fn load_fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/fixtures/openai/{}", name);
    std::fs::read(path).unwrap()
}

#[test]
fn test_parse_chat_request() {
    let body = load_fixture("chat_request.json");
    let adapter = OpenAIAdapter;
    let req = adapter.parse_request(&body).unwrap();

    assert_eq!(req.model, "gpt-4o");
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].role, AIRole::System);
    assert_eq!(req.messages[1].role, AIRole::User);
    assert!(!req.stream);
    assert_eq!(req.temperature, Some(0.7));
    assert_eq!(req.max_tokens, Some(256));
}

#[test]
fn test_parse_streaming_request() {
    let body = load_fixture("chat_request_stream.json");
    let adapter = OpenAIAdapter;
    let req = adapter.parse_request(&body).unwrap();

    assert!(req.stream);
    assert_eq!(req.messages.len(), 1);
}

#[test]
fn test_serialize_response() {
    let ai_resp = AIResponse {
        id: "chatcmpl-test".into(),
        model: "gpt-4o".into(),
        choices: vec![AIChoice {
            index: 0,
            message: AIMessage {
                role: AIRole::Assistant,
                content: AIContent::Text("Hello!".into()),
                name: None,
                tool_calls: vec![],
                tool_call_id: None,
            },
            finish_reason: Some("stop".into()),
        }],
        usage: Some(AIUsage {
            prompt_tokens: 10,
            completion_tokens: 2,
            total_tokens: 12,
        }),
        created: Some(1700000000),
        extra: Default::default(),
    };

    let adapter = OpenAIAdapter;
    let body = adapter.serialize_response(&ai_resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(parsed["object"], "chat.completion");
    assert_eq!(parsed["model"], "gpt-4o");
    assert_eq!(parsed["choices"][0]["message"]["role"], "assistant");
    assert_eq!(parsed["choices"][0]["message"]["content"], "Hello!");
    assert_eq!(parsed["choices"][0]["finish_reason"], "stop");
    assert_eq!(parsed["usage"]["total_tokens"], 12);
}

#[test]
fn test_serialize_stream_chunk() {
    let chunk = AIStreamChunk {
        id: "chatcmpl-123".into(),
        model: "gpt-4o".into(),
        choices: vec![AIStreamChoice {
            index: 0,
            delta: AIStreamDelta {
                role: Some(AIRole::Assistant),
                content: Some("Hello".into()),
                tool_calls: vec![],
            },
            finish_reason: None,
        }],
        usage: None,
        created: Some(1700000000),
    };

    let adapter = OpenAIAdapter;
    let sse_line = adapter.serialize_stream_chunk(&chunk).unwrap();

    assert!(sse_line.starts_with("data: "));
    assert!(sse_line.ends_with("\n\n"));

    let json_str = &sse_line[6..sse_line.len() - 2];
    let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();
    assert_eq!(parsed["object"], "chat.completion.chunk");
    assert_eq!(parsed["choices"][0]["delta"]["content"], "Hello");
}

#[test]
fn test_error_response() {
    let adapter = OpenAIAdapter;
    let body = adapter.error_response(429, "Rate limit exceeded").unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(parsed["error"]["message"], "Rate limit exceeded");
    assert_eq!(parsed["error"]["type"], "invalid_request_error");
    assert_eq!(parsed["error"]["code"], 429);
}

#[test]
fn test_roundtrip_request_response() {
    let body = load_fixture("chat_request.json");
    let adapter = OpenAIAdapter;

    let req = adapter.parse_request(&body).unwrap();
    let resp = AIResponse {
        id: "chatcmpl-roundtrip".into(),
        model: req.model.clone(),
        choices: vec![AIChoice {
            index: 0,
            message: AIMessage {
                role: AIRole::Assistant,
                content: AIContent::Text("Paris".into()),
                name: None,
                tool_calls: vec![],
                tool_call_id: None,
            },
            finish_reason: Some("stop".into()),
        }],
        usage: Some(AIUsage {
            prompt_tokens: 20,
            completion_tokens: 1,
            total_tokens: 21,
        }),
        created: None,
        extra: Default::default(),
    };

    let body = adapter.serialize_response(&resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(parsed["model"], "gpt-4o");
    assert_eq!(parsed["choices"][0]["message"]["content"], "Paris");
}

#[test]
fn test_parse_invalid_json() {
    let adapter = OpenAIAdapter;
    let result = adapter.parse_request(b"not json");
    assert!(matches!(result, Err(AIError::FormatParse { .. })));
}
