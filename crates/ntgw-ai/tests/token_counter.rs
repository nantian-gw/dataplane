use ntgw_ai::format::ir::*;
use ntgw_ai::token::TokenCounter;

#[test]
fn test_record_response() {
    let mut tc = TokenCounter::new();
    tc.record_response(&AIUsage {
        prompt_tokens: 50,
        completion_tokens: 30,
        total_tokens: 80,
    });

    let usage = tc.accumulated_usage();
    assert_eq!(usage.prompt_tokens, 50);
    assert_eq!(usage.completion_tokens, 30);
    assert_eq!(usage.total_tokens, 80);
}

#[test]
fn test_accumulate_stream_chunks() {
    let mut tc = TokenCounter::new();

    // First two chunks have no usage
    tc.record_stream_chunk(&AIStreamChunk {
        id: "1".into(),
        model: "gpt-4".into(),
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
    });

    tc.record_stream_chunk(&AIStreamChunk {
        id: "2".into(),
        model: "gpt-4".into(),
        choices: vec![AIStreamChoice {
            index: 0,
            delta: AIStreamDelta {
                role: None,
                content: Some(" world".into()),
                tool_calls: vec![],
            },
            finish_reason: None,
        }],
        usage: None,
        created: None,
    });

    // Last chunk carries usage
    tc.record_stream_chunk(&AIStreamChunk {
        id: "3".into(),
        model: "gpt-4".into(),
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
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        }),
        created: None,
    });

    let usage = tc.accumulated_usage();
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);
}

#[test]
fn test_no_usage() {
    let mut tc = TokenCounter::new();

    tc.record_stream_chunk(&AIStreamChunk {
        id: "1".into(),
        model: "gpt-4".into(),
        choices: vec![AIStreamChoice {
            index: 0,
            delta: AIStreamDelta {
                role: None,
                content: Some("Hi".into()),
                tool_calls: vec![],
            },
            finish_reason: None,
        }],
        usage: None,
        created: None,
    });

    tc.record_stream_chunk(&AIStreamChunk {
        id: "2".into(),
        model: "gpt-4".into(),
        choices: vec![AIStreamChoice {
            index: 0,
            delta: AIStreamDelta {
                role: None,
                content: Some(" there".into()),
                tool_calls: vec![],
            },
            finish_reason: Some("stop".into()),
        }],
        usage: None,
        created: None,
    });

    let usage = tc.accumulated_usage();
    assert_eq!(usage.prompt_tokens, 0);
    assert_eq!(usage.completion_tokens, 0);
    assert_eq!(usage.total_tokens, 0);
}

#[test]
fn test_parse_sse_body() {
    let body = concat!(
        "data: {\"id\":\"1\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}],\"usage\":null}\n\n",
        "data: {\"id\":\"2\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world\"}}],\"usage\":null}\n\n",
        "data: {\"id\":\"3\",\"model\":\"gpt-4\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\n",
        "data: [DONE]\n\n",
    );

    let (usage, content) = TokenCounter::from_sse_body(body.as_bytes()).unwrap();

    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 15);
    assert_eq!(content, "Hello world");
}
