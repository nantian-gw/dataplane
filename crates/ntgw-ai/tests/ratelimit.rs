use ntgw_ai::ratelimit::{RateLimitConfig, RateLimitResult, TokenRateLimiter};

#[test]
fn test_allowed_when_no_limit() {
    let config = RateLimitConfig::default(); // all zeros = unlimited
    let rl = TokenRateLimiter::new(config);
    assert!(matches!(rl.check("key1"), RateLimitResult::Allowed { .. }));
    assert!(matches!(
        rl.record_usage("key1", 1000000),
        RateLimitResult::Allowed { .. }
    ));
}

#[test]
fn test_request_limit_exceeded() {
    let config = RateLimitConfig {
        requests_per_minute: 2,
        burst: 1.0,
        ..Default::default()
    };
    let rl = TokenRateLimiter::new(config);
    assert!(matches!(rl.check("key1"), RateLimitResult::Allowed { .. }));
    assert!(matches!(rl.check("key1"), RateLimitResult::Allowed { .. }));
    assert!(matches!(rl.check("key1"), RateLimitResult::Limited { .. }));
}

#[test]
fn test_request_limit_remaining_ignores_disabled_token_limits() {
    let config = RateLimitConfig {
        requests_per_minute: 2,
        burst: 1.0,
        ..Default::default()
    };
    let rl = TokenRateLimiter::new(config);

    assert_eq!(rl.check("key1"), RateLimitResult::Allowed { remaining: 1 });
    assert_eq!(rl.check("key1"), RateLimitResult::Allowed { remaining: 0 });
}

#[test]
fn test_limited_retry_after_does_not_panic() {
    let config = RateLimitConfig {
        requests_per_minute: 1,
        burst: 1.0,
        ..Default::default()
    };
    let rl = TokenRateLimiter::new(config);

    assert!(matches!(rl.check("key1"), RateLimitResult::Allowed { .. }));
    let limited = rl.check("key1");
    assert!(matches!(
        limited,
        RateLimitResult::Limited {
            retry_after_secs: 1..=60
        }
    ));
}

#[test]
fn test_token_limit_exceeded() {
    let config = RateLimitConfig {
        tokens_per_minute: 100,
        burst: 1.0,
        ..Default::default()
    };
    let rl = TokenRateLimiter::new(config);
    rl.check("key1");
    let result = rl.record_usage("key1", 150);
    assert!(
        matches!(result, RateLimitResult::Limited { .. }),
        "expected Limited, got {:?}",
        result
    );
}

#[test]
fn test_multi_key_independent() {
    let config = RateLimitConfig {
        tokens_per_minute: 100,
        burst: 1.0,
        ..Default::default()
    };
    let rl = TokenRateLimiter::new(config);
    rl.check("key1");
    rl.record_usage("key1", 80);
    // key2 should not be affected by key1's usage
    rl.check("key2");
    assert!(matches!(
        rl.record_usage("key2", 50),
        RateLimitResult::Allowed { .. }
    ));
}

#[test]
fn test_window_reset() {
    let config = RateLimitConfig {
        requests_per_minute: 1,
        burst: 1.0,
        ..Default::default()
    };
    let rl = TokenRateLimiter::new(config);
    rl.check("key1");
    assert!(matches!(rl.check("key1"), RateLimitResult::Limited { .. }));
}

#[test]
fn test_hour_token_limit() {
    let config = RateLimitConfig {
        tokens_per_hour: 200,
        burst: 1.0,
        ..Default::default()
    };
    let rl = TokenRateLimiter::new(config);
    rl.check("key1");
    let result = rl.record_usage("key1", 250);
    assert!(matches!(result, RateLimitResult::Limited { .. }));
}

#[test]
fn test_hour_only_remaining_ignores_disabled_minute_limit() {
    let config = RateLimitConfig {
        tokens_per_hour: 200,
        burst: 1.0,
        ..Default::default()
    };
    let rl = TokenRateLimiter::new(config);

    assert_eq!(
        rl.record_usage("key1", 75),
        RateLimitResult::Allowed { remaining: 125 }
    );
}

#[test]
fn test_burst_multiplier() {
    let config = RateLimitConfig {
        tokens_per_minute: 100,
        burst: 2.0, // 2x burst = 200 token limit
        ..Default::default()
    };
    let rl = TokenRateLimiter::new(config);
    rl.check("key1");
    // 150 should be allowed with 2x burst
    assert!(matches!(
        rl.record_usage("key1", 150),
        RateLimitResult::Allowed { .. }
    ));
    // But 250 should exceed even with burst
    rl.check("key1");
    assert!(matches!(
        rl.record_usage("key1", 250),
        RateLimitResult::Limited { .. }
    ));
}

#[test]
fn test_zero_limits_always_allowed() {
    let config = RateLimitConfig {
        tokens_per_minute: 0,
        tokens_per_hour: 0,
        requests_per_minute: 0,
        ..Default::default()
    };
    let rl = TokenRateLimiter::new(config);
    for _ in 0..1000 {
        assert!(matches!(rl.check("key1"), RateLimitResult::Allowed { .. }));
    }
    assert!(matches!(
        rl.record_usage("key1", u64::MAX),
        RateLimitResult::Allowed { .. }
    ));
}
