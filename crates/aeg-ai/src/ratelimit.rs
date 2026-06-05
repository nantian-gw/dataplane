use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Result of a rate limit check.
#[derive(Debug, Clone, PartialEq)]
pub enum RateLimitResult {
    /// Request is allowed, with remaining quota.
    Allowed { remaining: u64 },
    /// Request is rate-limited, with retry-after hint in seconds.
    Limited { retry_after_secs: u64 },
}

/// Token rate limiter configuration (matches proto TokenPolicyConfig).
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub tokens_per_minute: u64,
    pub tokens_per_hour: u64,
    pub requests_per_minute: u64,
    pub scope: String,    // apiKey, model, user
    pub burst: f64,       // multiplier, 1.0 = no burst
    pub on_limit: String, // reject, queue, warn
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            tokens_per_minute: 0,
            tokens_per_hour: 0,
            requests_per_minute: 0,
            scope: "apiKey".into(),
            burst: 1.0,
            on_limit: "reject".into(),
        }
    }
}

/// Per-key sliding window state.
struct SlidingWindow {
    minute_tokens: u64,
    hour_tokens: u64,
    minute_requests: u64,
    minute_reset: Instant,
    hour_reset: Instant,
    minute_limit: u64,
    hour_limit: u64,
    minute_req_limit: u64,
    #[allow(dead_code)]
    burst_factor: f64,
}

impl SlidingWindow {
    fn new(config: &RateLimitConfig) -> Self {
        let now = Instant::now();
        let burst = config.burst.max(1.0);
        Self {
            minute_tokens: 0,
            hour_tokens: 0,
            minute_requests: 0,
            minute_reset: now + Duration::from_secs(60),
            hour_reset: now + Duration::from_secs(3600),
            minute_limit: (config.tokens_per_minute as f64 * burst) as u64,
            hour_limit: (config.tokens_per_hour as f64 * burst) as u64,
            minute_req_limit: (config.requests_per_minute as f64 * burst) as u64,
            burst_factor: burst,
        }
    }

    /// Check before sending request. Returns whether request can proceed.
    fn check(&mut self) -> RateLimitResult {
        let now = Instant::now();

        // Reset windows if expired
        if now >= self.minute_reset {
            self.minute_tokens = 0;
            self.minute_requests = 0;
            self.minute_reset = now + Duration::from_secs(60);
        }
        if now >= self.hour_reset {
            self.hour_tokens = 0;
            self.hour_reset = now + Duration::from_secs(3600);
        }

        self.minute_requests += 1;

        // Check request limit (after incrementing, so we include this request)
        if self.minute_req_limit > 0 && self.minute_requests > self.minute_req_limit {
            let secs = self.minute_reset.duration_since(now).as_secs().max(1);
            return RateLimitResult::Limited {
                retry_after_secs: secs,
            };
        }

        RateLimitResult::Allowed { remaining: 0 }
    }

    /// Record tokens after response and check limits.
    fn record_tokens(&mut self, tokens: u64) -> RateLimitResult {
        let now = Instant::now();
        self.minute_tokens += tokens;
        self.hour_tokens += tokens;

        // Check after recording
        if self.hour_limit > 0 && self.hour_tokens > self.hour_limit {
            let secs = self.hour_reset.duration_since(now).as_secs().max(1);
            return RateLimitResult::Limited {
                retry_after_secs: secs,
            };
        }
        if self.minute_limit > 0 && self.minute_tokens > self.minute_limit {
            let secs = self.minute_reset.duration_since(now).as_secs().max(1);
            return RateLimitResult::Limited {
                retry_after_secs: secs,
            };
        }

        RateLimitResult::Allowed {
            remaining: self.minute_limit.saturating_sub(self.minute_tokens),
        }
    }
}

/// Thread-safe token/request rate limiter with per-key sliding windows.
pub struct TokenRateLimiter {
    windows: Mutex<HashMap<String, SlidingWindow>>,
    pub config: RateLimitConfig,
}

impl TokenRateLimiter {
    /// Create a new rate limiter with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            windows: Mutex::new(HashMap::new()),
            config,
        }
    }

    /// Check before sending request. Returns whether request can proceed.
    ///
    /// When all limits are zero (unlimited), always returns `Allowed`.
    #[allow(clippy::unwrap_used)]
    pub fn check(&self, key: &str) -> RateLimitResult {
        if self.config.tokens_per_minute == 0
            && self.config.tokens_per_hour == 0
            && self.config.requests_per_minute == 0
        {
            return RateLimitResult::Allowed {
                remaining: u64::MAX,
            };
        }

        let mut windows = self.windows.lock().unwrap();
        let window = windows
            .entry(key.to_string())
            .or_insert_with(|| SlidingWindow::new(&self.config));
        window.check()
    }

    /// Record token usage after response and check token limits.
    ///
    /// When all limits are zero (unlimited), always returns `Allowed`.
    #[allow(clippy::unwrap_used)]
    pub fn record_usage(&self, key: &str, total_tokens: u64) -> RateLimitResult {
        if self.config.tokens_per_minute == 0 && self.config.tokens_per_hour == 0 {
            return RateLimitResult::Allowed {
                remaining: u64::MAX,
            };
        }

        let mut windows = self.windows.lock().unwrap();
        let window = windows
            .entry(key.to_string())
            .or_insert_with(|| SlidingWindow::new(&self.config));
        window.record_tokens(total_tokens)
    }
}
