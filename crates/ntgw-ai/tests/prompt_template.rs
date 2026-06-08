use ntgw_ai::format::ir::{AIRequest, AIRole};
use ntgw_ai::prompt_template::{PromptInjector, PromptTemplate};

fn make_request(model: &str) -> AIRequest {
    AIRequest {
        messages: Vec::new(),
        model: model.to_string(),
        temperature: None,
        max_tokens: None,
        top_p: None,
        stop: Vec::new(),
        stream: false,
        user: None,
        extra: std::collections::BTreeMap::new(),
    }
}

#[test]
fn inject_system_prompt() {
    let mut injector = PromptInjector::new();
    let template = PromptTemplate::new("support", "You are a helpful support agent.");
    injector.register(template);

    let mut request = make_request("gpt-4");
    let injected = injector.inject("support", &mut request);

    assert!(injected);
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.messages[0].role, AIRole::System);
    if let ntgw_ai::format::ir::AIContent::Text(ref t) = request.messages[0].content {
        assert_eq!(t, "You are a helpful support agent.");
    } else {
        panic!("expected text content");
    }
}

#[test]
fn inject_with_variable_resolution() {
    let mut injector = PromptInjector::new();
    let template = PromptTemplate::new("greet", "Hello {name}, welcome to the agent.")
        .with_variable("name", "Alice");
    injector.register(template);

    let mut request = make_request("gpt-4");
    injector.inject("greet", &mut request);

    if let ntgw_ai::format::ir::AIContent::Text(ref t) = request.messages[0].content {
        assert_eq!(t, "Hello Alice, welcome to the agent.");
    } else {
        panic!("expected text content");
    }
}

#[test]
fn inject_few_shot_examples() {
    let mut injector = PromptInjector::new();
    let mut template = PromptTemplate::new("qa", "You answer questions concisely.");
    template.add_example(AIRole::User, "What is 2+2?");
    template.add_example(AIRole::Assistant, "4");
    injector.register(template);

    let mut request = make_request("gpt-4");
    injector.inject("qa", &mut request);

    assert_eq!(request.messages.len(), 3);
    assert_eq!(request.messages[0].role, AIRole::System);
    assert_eq!(request.messages[1].role, AIRole::User);
    assert_eq!(request.messages[2].role, AIRole::Assistant);

    if let ntgw_ai::format::ir::AIContent::Text(ref t) = request.messages[1].content {
        assert_eq!(t, "What is 2+2?");
    }
    if let ntgw_ai::format::ir::AIContent::Text(ref t) = request.messages[2].content {
        assert_eq!(t, "4");
    }
}

#[test]
fn inject_preserves_existing_messages() {
    let mut injector = PromptInjector::new();
    injector.register(PromptTemplate::new("sys", "System prompt."));

    let mut request = make_request("gpt-4");
    request.messages.push(ntgw_ai::format::ir::AIMessage {
        role: AIRole::User,
        content: ntgw_ai::format::ir::AIContent::Text("real question".into()),
        name: None,
        tool_calls: Vec::new(),
        tool_call_id: None,
    });

    injector.inject("sys", &mut request);

    assert_eq!(request.messages.len(), 2);
    assert_eq!(request.messages[0].role, AIRole::System);
    assert_eq!(request.messages[1].role, AIRole::User);
    if let ntgw_ai::format::ir::AIContent::Text(ref t) = request.messages[1].content {
        assert_eq!(t, "real question");
    }
}

#[test]
fn inject_empty_template() {
    let injector = PromptInjector::new();
    let mut request = make_request("gpt-4");
    let injected = injector.inject("unknown", &mut request);

    assert!(!injected);
    assert!(request.messages.is_empty());
}

#[test]
fn injector_default_empty() {
    let injector = PromptInjector::new();
    assert!(injector.is_empty());
    assert_eq!(injector.len(), 0);
}
