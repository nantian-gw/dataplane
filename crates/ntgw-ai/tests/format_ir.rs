use ntgw_ai::format::{AIContent, AIMessage, AIRequest, AIResponse, AIRole, AIStreamChunk, AIUsage};

#[test]
fn test_airequest_simple_roundtrip() {
    let json = r#"{
        "model": "gpt-4o",
        "messages": [
            {"role": "system", "content": "You are helpful."},
            {"role": "user", "content": "Hello"}
        ],
        "temperature": 0.7,
        "stream": false
    }"#;

    let req: AIRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.model, "gpt-4o");
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].role, AIRole::System);
    assert_eq!(req.messages[1].role, AIRole::User);
    assert!(!req.stream);

    let roundtrip = serde_json::to_string(&req).unwrap();
    let req2: AIRequest = serde_json::from_str(&roundtrip).unwrap();
    assert_eq!(req2.model, "gpt-4o");
    assert_eq!(req2.messages.len(), 2);
}

#[test]
fn test_airequest_with_tool_calls() {
    let json = r#"{
        "model": "gpt-4o",
        "messages": [
            {"role": "user", "content": "What is 2+2?"},
            {
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    {
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "calculator",
                            "arguments": "{\"expression\":\"2+2\"}"
                        }
                    }
                ]
            },
            {
                "role": "tool",
                "content": "4",
                "tool_call_id": "call_1"
            }
        ]
    }"#;

    let req: AIRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.messages.len(), 3);

    let assistant = &req.messages[1];
    assert_eq!(assistant.role, AIRole::Assistant);
    assert_eq!(assistant.tool_calls.len(), 1);
    assert_eq!(assistant.tool_calls[0].id, "call_1");
    assert_eq!(assistant.tool_calls[0].function.name, "calculator");

    let roundtrip = serde_json::to_string(&req).unwrap();
    let req2: AIRequest = serde_json::from_str(&roundtrip).unwrap();
    assert_eq!(req2.messages.len(), 3);
    assert_eq!(req2.messages[1].tool_calls.len(), 1);
}

#[test]
fn test_aimessage_text_content() {
    let json = r#"{"role": "user", "content": "Hello world"}"#;
    let msg: AIMessage = serde_json::from_str(json).unwrap();
    assert_eq!(msg.role, AIRole::User);
    match msg.content {
        AIContent::Text(text) => assert_eq!(text, "Hello world"),
        _ => panic!("expected Text content"),
    }
}

#[test]
fn test_aimessage_multipart_content() {
    let json = r#"{
        "role": "user",
        "content": [
            {"type": "text", "text": "What is in this image?"},
            {"type": "image_url", "image_url": {"url": "https://example.com/img.jpg"}}
        ]
    }"#;
    let msg: AIMessage = serde_json::from_str(json).unwrap();
    match msg.content {
        AIContent::MultiPart(parts) => {
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0].content_type, "text");
            assert_eq!(parts[1].content_type, "image_url");
        }
        _ => panic!("expected MultiPart content"),
    }
}

#[test]
fn test_airesponse_roundtrip() {
    let json = r#"{
        "id": "chatcmpl-123",
        "model": "gpt-4o",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help?"
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        },
        "created": 1700000000
    }"#;

    let resp: AIResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.id, "chatcmpl-123");
    assert_eq!(resp.model, "gpt-4o");
    assert_eq!(resp.choices.len(), 1);
    assert_eq!(resp.choices[0].finish_reason.as_deref(), Some("stop"));

    let usage = resp.usage.clone().unwrap();
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 15);

    let roundtrip = serde_json::to_string(&resp).unwrap();
    let resp2: AIResponse = serde_json::from_str(&roundtrip).unwrap();
    assert_eq!(resp2.id, "chatcmpl-123");
    assert_eq!(resp2.usage.as_ref().unwrap().total_tokens, 15);
}

#[test]
fn test_aistream_chunk_roundtrip() {
    let json = r#"{
        "id": "chatcmpl-123",
        "model": "gpt-4o",
        "choices": [
            {
                "index": 0,
                "delta": {
                    "role": "assistant",
                    "content": "Hello"
                },
                "finish_reason": null
            }
        ]
    }"#;

    let chunk: AIStreamChunk = serde_json::from_str(json).unwrap();
    assert_eq!(chunk.id, "chatcmpl-123");
    assert_eq!(chunk.choices.len(), 1);
    assert_eq!(chunk.choices[0].delta.role, Some(AIRole::Assistant));
    assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("Hello"));
    assert!(chunk.usage.is_none());

    let roundtrip = serde_json::to_string(&chunk).unwrap();
    let chunk2: AIStreamChunk = serde_json::from_str(&roundtrip).unwrap();
    assert_eq!(chunk2.choices[0].delta.content.as_deref(), Some("Hello"));
}

#[test]
fn test_aistream_final_chunk_with_usage() {
    let json = r#"{
        "id": "chatcmpl-123",
        "model": "gpt-4o",
        "choices": [
            {
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    }"#;

    let chunk: AIStreamChunk = serde_json::from_str(json).unwrap();
    assert_eq!(chunk.choices[0].finish_reason.as_deref(), Some("stop"));
    assert!(chunk.choices[0].delta.content.is_none());

    let usage = chunk.usage.unwrap();
    assert_eq!(usage.total_tokens, 15);
}

#[test]
fn test_aiusage_default_zero() {
    let usage = AIUsage::default();
    assert_eq!(usage.prompt_tokens, 0);
    assert_eq!(usage.completion_tokens, 0);
    assert_eq!(usage.total_tokens, 0);
}
