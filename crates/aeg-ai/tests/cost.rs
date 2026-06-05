use aeg_ai::cost::*;
use aeg_ai::format::ir::AIUsage;

#[test]
fn test_calc_cost_gpt4o() {
    let tracker = CostTracker::new();
    let usage = AIUsage {
        prompt_tokens: 1000,
        completion_tokens: 500,
        total_tokens: 1500,
    };
    // 1000 input tokens * $2.50/1K = $2.50
    // 500 output tokens * $10.00/1K = $5.00
    // total = $7.50
    let cost = tracker.calc_cost("gpt-4o", &usage);
    assert!((cost - 7.50).abs() < 0.01, "expected ~$7.50, got ${cost}");
}

#[test]
fn test_record_accumulates() {
    let tracker = CostTracker::new();
    let usage = AIUsage {
        prompt_tokens: 1000,
        completion_tokens: 0,
        total_tokens: 1000,
    };
    tracker.record("gpt-4o", &usage);
    tracker.record("gpt-4o", &usage);
    let total = tracker.total_cost_dollars();
    // 2 * $2.50 = $5.00
    assert!((total - 5.00).abs() < 0.01);
}

#[test]
fn test_unknown_model_returns_zero() {
    let tracker = CostTracker::new();
    let usage = AIUsage {
        prompt_tokens: 1_000_000,
        completion_tokens: 1_000_000,
        total_tokens: 2_000_000,
    };
    let cost = tracker.calc_cost("unknown-model", &usage);
    assert_eq!(cost, 0.0);
}

#[test]
fn test_reset() {
    let tracker = CostTracker::new();
    tracker.record(
        "gpt-4o",
        &AIUsage {
            prompt_tokens: 1000,
            completion_tokens: 0,
            total_tokens: 1000,
        },
    );
    tracker.reset();
    assert_eq!(tracker.total_cost_dollars(), 0.0);
}

#[test]
fn test_custom_pricing() {
    let mut pricing = std::collections::HashMap::new();
    pricing.insert("my-model".to_string(), ModelPricing::new(1.0, 2.0));
    let tracker = CostTracker::with_pricing(pricing);
    let usage = AIUsage {
        prompt_tokens: 2000,
        completion_tokens: 500,
        total_tokens: 2500,
    };
    // 2000 * $1.00/1K = $2.00  +  500 * $2.00/1K = $1.00  => $3.00
    let cost = tracker.calc_cost("my-model", &usage);
    assert!((cost - 3.00).abs() < 0.01, "expected ~$3.00, got ${cost}");
}

#[test]
fn test_zero_tokens_zero_cost() {
    let tracker = CostTracker::new();
    let usage = AIUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
    };
    let cost = tracker.calc_cost("gpt-4o", &usage);
    assert_eq!(cost, 0.0);
}
