use ntgw_ai::error::AIError;
use ntgw_ai::format::FormatAdapter;
use ntgw_ai::format::ir::*;
use ntgw_ai::format::ollama::OllamaAdapter;

fn load_fixture(name: &str) -> Vec<u8> {
    let path = format!("tests/fixtures/ollama/{}", name);
    std::fs::read(path).unwrap()
}

#[test]
fn test_parse_with_options() {
    let body = load_fixture("chat_request.json");
    let adapter = OllamaAdapter;
    let req = adapter.parse_request(&body).unwrap();

    assert_eq!(req.model, "llama3.2");
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].role, AIRole::System);
    assert_eq!(req.messages[1].role, AIRole::User);
    assert_eq!(req.temperature, Some(0.5));
    assert_eq!(req.max_tokens, Some(100));
    assert!(!req.stream);
}

#[test]
fn test_parse_basic() {
    let body = load_fixture("chat_request_basic.json");
    let adapter = OllamaAdapter;
    let req = adapter.parse_request(&body).unwrap();

    assert_eq!(req.model, "llama3.2");
    assert_eq!(req.messages.len(), 1);
    assert!(req.temperature.is_none());
    assert!(req.max_tokens.is_none());
}

#[test]
fn test_serialize_response() {
    let ai_resp = AIResponse {
        id: "".into(),
        model: "llama3.2".into(),
        choices: vec![AIChoice {
            index: 0,
            message: AIMessage {
                role: AIRole::Assistant,
                content: AIContent::Text("4".into()),
                name: None,
                tool_calls: vec![],
                tool_call_id: None,
            },
            finish_reason: None,
        }],
        usage: Some(AIUsage {
            prompt_tokens: 10,
            completion_tokens: 1,
            total_tokens: 11,
        }),
        created: None,
        extra: Default::default(),
    };

    let adapter = OllamaAdapter;
    let body = adapter.serialize_response(&ai_resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(parsed["model"], "llama3.2");
    assert_eq!(parsed["message"]["role"], "assistant");
    assert_eq!(parsed["message"]["content"], "4");
    assert_eq!(parsed["done"], true);
    assert_eq!(parsed["eval_count"], 1);
    assert_eq!(parsed["prompt_eval_count"], 10);
}

#[test]
fn test_serialize_stream_chunk() {
    let chunk = AIStreamChunk {
        id: "".into(),
        model: "llama3.2".into(),
        choices: vec![AIStreamChoice {
            index: 0,
            delta: AIStreamDelta {
                role: None,
                content: Some("4".into()),
                tool_calls: vec![],
            },
            finish_reason: None,
        }],
        usage: None,
        created: None,
    };

    let adapter = OllamaAdapter;
    let line = adapter.serialize_stream_chunk(&chunk).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(parsed["message"]["content"], "4");
    assert_eq!(parsed["done"], false);
}

#[test]
fn test_stream_final_chunk() {
    let chunk = AIStreamChunk {
        id: "".into(),
        model: "llama3.2".into(),
        choices: vec![AIStreamChoice {
            index: 0,
            delta: AIStreamDelta {
                role: None,
                content: None,
                tool_calls: vec![],
            },
            finish_reason: Some("stop".into()),
        }],
        usage: Some(AIUsage {
            prompt_tokens: 10,
            completion_tokens: 1,
            total_tokens: 11,
        }),
        created: None,
    };

    let adapter = OllamaAdapter;
    let line = adapter.serialize_stream_chunk(&chunk).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(parsed["done"], true);
}

#[test]
fn test_parse_stream_body_reads_ollama_json_lines() {
    let adapter = OllamaAdapter;
    let body = br#"{"model":"llama3.2","created_at":"","message":{"role":"assistant","content":"Hel"},"done":false}
{"model":"llama3.2","created_at":"","message":{"role":"assistant","content":"lo"},"done":false}
{"model":"llama3.2","created_at":"","message":{"role":"assistant","content":""},"done":true,"prompt_eval_count":10,"eval_count":2}
"#;

    let chunks = adapter
        .parse_stream_body(body)
        .expect("ollama stream body should parse");

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].model, "llama3.2");
    assert_eq!(chunks[0].choices[0].delta.content.as_deref(), Some("Hel"));
    assert_eq!(chunks[1].choices[0].delta.content.as_deref(), Some("lo"));
    assert_eq!(chunks[2].choices[0].finish_reason.as_deref(), Some("stop"));
    assert_eq!(
        chunks[2].usage.as_ref().map(|usage| usage.total_tokens),
        Some(12)
    );
}

#[test]
fn test_error_response() {
    let adapter = OllamaAdapter;
    let body = adapter.error_response(500, "Internal error").unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(parsed["error"], "Internal error");
}

#[test]
fn test_parse_invalid_json() {
    let adapter = OllamaAdapter;
    let result = adapter.parse_request(b"not json");
    assert!(matches!(result, Err(AIError::FormatParse { .. })));
}
