use std::collections::BTreeMap;

use ntgw_ai::format::ir::{AIContent, AIMessage, AIRequest, AIRole};
use ntgw_ai::model_router::{Complexity, ModelRoute, ModelRouter};

fn make_request(messages: Vec<(AIRole, String)>) -> AIRequest {
    AIRequest {
        messages: messages
            .into_iter()
            .map(|(role, content)| AIMessage {
                role,
                content: AIContent::Text(content),
                name: None,
                tool_calls: Vec::new(),
                tool_call_id: None,
            })
            .collect(),
        model: "test-model".into(),
        temperature: None,
        max_tokens: None,
        top_p: None,
        stop: Vec::new(),
        stream: false,
        user: None,
        extra: BTreeMap::new(),
    }
}

#[test]
fn classify_simple() {
    let router = ModelRouter::new();
    let req = make_request(vec![(AIRole::User, "hello".into())]);
    assert_eq!(router.classify(&req), Complexity::Simple);
}

#[test]
fn classify_medium() {
    let router = ModelRouter::new();
    let medium_msg = "x".repeat(500);
    let req = make_request(vec![(AIRole::User, medium_msg)]);
    assert_eq!(router.classify(&req), Complexity::Medium);
}

#[test]
fn classify_complex() {
    let router = ModelRouter::new();
    let long_msg = "x".repeat(2500);
    let req = make_request(vec![(AIRole::User, long_msg)]);
    assert_eq!(router.classify(&req), Complexity::Complex);
}

#[test]
fn classify_boundary_simple_to_medium() {
    let router = ModelRouter::new();
    let msg = "x".repeat(199);
    let req = make_request(vec![(AIRole::User, msg)]);
    assert_eq!(router.classify(&req), Complexity::Simple);

    let msg = "x".repeat(200);
    let req = make_request(vec![(AIRole::User, msg)]);
    assert_eq!(router.classify(&req), Complexity::Medium);
}

#[test]
fn classify_boundary_medium_to_complex() {
    let router = ModelRouter::new();
    let msg = "x".repeat(1999);
    let req = make_request(vec![(AIRole::User, msg)]);
    assert_eq!(router.classify(&req), Complexity::Medium);

    let msg = "x".repeat(2000);
    let req = make_request(vec![(AIRole::User, msg)]);
    assert_eq!(router.classify(&req), Complexity::Complex);
}

#[test]
fn classify_multiple_messages() {
    let router = ModelRouter::new();
    let req = make_request(vec![
        (AIRole::System, "x".repeat(100)),
        (AIRole::User, "x".repeat(100)),
    ]);
    assert_eq!(router.classify(&req), Complexity::Medium);
}

#[test]
fn route_returns_best_model() {
    let mut router = ModelRouter::new();
    router.add_routes(
        Complexity::Simple,
        vec![ModelRoute::new("gpt-3.5-turbo", 10, Some(4096))],
    );

    let best = router.route(Complexity::Simple).unwrap();
    assert_eq!(best.model, "gpt-3.5-turbo");
    assert_eq!(best.weight, 10);
}

#[test]
fn route_no_routes_for_complexity() {
    let router = ModelRouter::new();
    assert!(router.route(Complexity::Complex).is_none());
}

#[test]
fn classify_and_route() {
    let mut router = ModelRouter::new();
    router.add_routes(
        Complexity::Simple,
        vec![ModelRoute::new("fast-model", 10, None)],
    );
    router.add_routes(
        Complexity::Medium,
        vec![ModelRoute::new("balanced-model", 10, None)],
    );
    router.add_routes(
        Complexity::Complex,
        vec![ModelRoute::new("powerful-model", 10, None)],
    );

    let simple_req = make_request(vec![(AIRole::User, "hi".into())]);
    assert_eq!(
        router.classify_and_route(&simple_req).unwrap().model,
        "fast-model"
    );

    let medium_req = make_request(vec![(AIRole::User, "x".repeat(500))]);
    assert_eq!(
        router.classify_and_route(&medium_req).unwrap().model,
        "balanced-model"
    );

    let complex_req = make_request(vec![(AIRole::User, "x".repeat(3000))]);
    assert_eq!(
        router.classify_and_route(&complex_req).unwrap().model,
        "powerful-model"
    );
}

#[test]
fn router_default_empty() {
    let router = ModelRouter::new();
    assert!(router.is_empty());
    assert_eq!(router.len(), 0);
}
