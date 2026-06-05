use aeg_ai::semantic_cache::*;

fn make_request(content: &str) -> aeg_ai::format::ir::AIRequest {
    use aeg_ai::format::ir::*;
    AIRequest {
        messages: vec![AIMessage {
            role: AIRole::User,
            content: AIContent::Text(content.into()),
            name: None,
            tool_calls: vec![],
            tool_call_id: None,
        }],
        model: "gpt-4o".into(),
        temperature: None,
        max_tokens: None,
        top_p: None,
        stop: vec![],
        stream: false,
        user: None,
        extra: Default::default(),
    }
}

fn make_response(id: &str, content: &str) -> aeg_ai::format::ir::AIResponse {
    use aeg_ai::format::ir::*;
    AIResponse {
        id: id.into(),
        model: "gpt-4o".into(),
        choices: vec![AIChoice {
            index: 0,
            message: AIMessage {
                role: AIRole::Assistant,
                content: AIContent::Text(content.into()),
                name: None,
                tool_calls: vec![],
                tool_call_id: None,
            },
            finish_reason: Some("stop".into()),
        }],
        usage: Some(AIUsage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        }),
        created: None,
        extra: Default::default(),
    }
}

#[test]
fn test_cache_hit() {
    let cache = SemanticCache::with_memory_backend(CacheConfig::default());
    let req = make_request("What is Rust?");
    let resp = make_response("1", "Rust is a systems programming language.");

    cache.store(&req, &resp);
    let result = cache.lookup(&req);
    assert!(result.is_some());
    assert_eq!(result.unwrap().id, "1");
}

#[test]
fn test_cache_miss() {
    let cache = SemanticCache::with_memory_backend(CacheConfig::default());
    let req = make_request("What is Rust?");
    let result = cache.lookup(&req);
    assert!(result.is_none());
}

#[test]
fn test_cache_disabled() {
    let config = CacheConfig {
        enabled: false,
        ..Default::default()
    };
    let cache = SemanticCache::with_memory_backend(config);
    let req = make_request("What is Rust?");
    let resp = make_response("1", "Rust is a systems language.");

    cache.store(&req, &resp);
    assert!(cache.lookup(&req).is_none());
}

#[test]
fn test_different_requests_different_keys() {
    let cache = SemanticCache::with_memory_backend(CacheConfig::default());
    let req1 = make_request("What is Rust?");
    let req2 = make_request("What is Python?");
    let resp = make_response("1", "A language.");

    cache.store(&req1, &resp);
    assert!(cache.lookup(&req2).is_none());
}
