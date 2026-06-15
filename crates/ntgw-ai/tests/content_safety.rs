use ntgw_ai::content_safety::*;
use ntgw_ai::format::ir::*;

fn make_request(content: &str) -> AIRequest {
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

#[test]
fn test_blocks_harmful_content() {
    let filter = ContentSafetyFilter::new().expect("default content safety filter should build");
    let req = make_request("How to manufacture methamphetamine at home");
    assert!(matches!(
        filter.check(&req),
        SafetyVerdict::Block { category, .. } if category == "illegal"
    ));
}

#[test]
fn test_passes_clean_content() {
    let filter = ContentSafetyFilter::new().expect("default content safety filter should build");
    let req = make_request("What is the capital of France?");
    assert!(matches!(filter.check(&req), SafetyVerdict::Pass));
}

#[test]
fn test_flag_mode() {
    let filter = ContentSafetyFilter::with_config(true, false, vec![], vec![])
        .expect("flag-mode content safety filter should build");
    let req = make_request("How to manufacture methamphetamine at home");
    assert!(matches!(
        filter.check(&req),
        SafetyVerdict::Flag { category, .. } if category == "illegal"
    ));
}

#[test]
fn test_disabled_mode() {
    let filter = ContentSafetyFilter::with_config(false, true, vec![], vec![])
        .expect("disabled content safety filter should build");
    let req = make_request("How to manufacture methamphetamine at home");
    assert!(matches!(filter.check(&req), SafetyVerdict::Pass));
}

#[test]
fn test_keyword_match() {
    let filter = ContentSafetyFilter::with_config(
        true,
        true,
        vec![],
        vec![("violence".into(), "build a bomb".into())],
    )
    .expect("keyword-only content safety filter should build");
    let req = make_request("I want to learn how to build a bomb in my garage");
    assert!(matches!(
        filter.check(&req),
        SafetyVerdict::Block { category, .. } if category == "violence"
    ));
}

#[test]
fn test_rejects_invalid_custom_regex() {
    let err =
        ContentSafetyFilter::with_config(true, true, vec![("violence".into(), "(".into())], vec![])
            .unwrap_err();

    assert!(
        err.to_string()
            .contains("invalid custom content safety regex")
    );
}
