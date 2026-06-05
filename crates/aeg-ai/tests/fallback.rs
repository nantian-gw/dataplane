use aeg_ai::fallback::*;

fn make_chain() -> ModelFallback {
    let mut mf = ModelFallback::new();
    mf.add_chain(FallbackChain {
        primary: "gpt-4o".into(),
        fallbacks: vec![
            FallbackEntry {
                model: "gpt-4o-mini".into(),
                on_status: vec![429, 500, 502, 503],
                on_timeout: false,
                max_retries: 1,
            },
            FallbackEntry {
                model: "claude-3-haiku".into(),
                on_status: vec![429, 500],
                on_timeout: true,
                max_retries: 1,
            },
        ],
    });
    mf
}

#[test]
fn test_fallback_on_429() {
    let mf = make_chain();
    let next = mf.resolve_fallback("gpt-4o", Some(429), false, 0);
    assert_eq!(next, Some("gpt-4o-mini"));
}

#[test]
fn test_no_fallback_on_200() {
    let mf = make_chain();
    let next = mf.resolve_fallback("gpt-4o", Some(200), false, 0);
    assert!(next.is_none());
}

#[test]
fn test_multi_step_fallback() {
    let mf = make_chain();
    let first = mf.resolve_fallback("gpt-4o", Some(503), false, 0);
    assert_eq!(first, Some("gpt-4o-mini"));
    let second = mf.resolve_fallback("gpt-4o", Some(500), true, 1);
    assert_eq!(second, Some("claude-3-haiku"));
}

#[test]
fn test_max_retries_exceeded() {
    let mf = make_chain();
    let result = mf.resolve_fallback("gpt-4o", Some(429), false, 2);
    assert!(result.is_none());
}

#[test]
fn test_unknown_model() {
    let mf = make_chain();
    let result = mf.resolve_fallback("unknown-model", Some(500), false, 0);
    assert!(result.is_none());
}
