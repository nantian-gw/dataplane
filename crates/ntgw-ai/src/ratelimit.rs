use dashmap::DashMap;
use std::collections::VecDeque;
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

/// Per-key sliding window state using true sliding-window algorithm.
/// Maintains deques of timestamps so that rate limits are enforced
/// continuously rather than on fixed clock boundaries.
struct SlidingWindow {
    minute_req_limit: u64,
    minute_token_limit: u64,
    hour_token_limit: u64,
    requests: VecDeque<Instant>,
    minute_tokens: VecDeque<(Instant, u64)>,
    hour_tokens: VecDeque<(Instant, u64)>,
}

impl SlidingWindow {
    fn new(config: &RateLimitConfig) -> Self {
        let burst = config.burst.max(1.0);
        Self {
            minute_req_limit: (config.requests_per_minute as f64 * burst) as u64,
            minute_token_limit: (config.tokens_per_minute as f64 * burst) as u64,
            hour_token_limit: (config.tokens_per_hour as f64 * burst) as u64,
            requests: VecDeque::new(),
            minute_tokens: VecDeque::new(),
            hour_tokens: VecDeque::new(),
        }
    }

    /// Check before sending request. Returns whether request can proceed.
    /// Also checks token budget from previous requests so that requests
    /// are rejected before incurring upstream cost when the budget is exhausted.
    fn check(&mut self) -> RateLimitResult {
        let now = Instant::now();
        let window = Duration::from_secs(60);

        // Remove requests and tokens outside the sliding window
        while let Some(&t) = self.requests.front() {
            if now.duration_since(t) > window {
                self.requests.pop_front();
            } else {
                break;
            }
        }
        while let Some(&(t, _)) = self.minute_tokens.front() {
            if now.duration_since(t) > window {
                self.minute_tokens.pop_front();
            } else {
                break;
            }
        }
        while let Some(&(t, _)) = self.hour_tokens.front() {
            if now.duration_since(t) > Duration::from_secs(3600) {
                self.hour_tokens.pop_front();
            } else {
                break;
            }
        }

        // Check minute token budget from previous requests
        let minute_used: u64 = self.minute_tokens.iter().map(|(_, t)| t).sum();
        if self.minute_token_limit > 0 && minute_used >= self.minute_token_limit {
            return RateLimitResult::Limited {
                retry_after_secs: 1,
            };
        }
        // Check hour token budget from previous requests
        let hour_used: u64 = self.hour_tokens.iter().map(|(_, t)| t).sum();
        if self.hour_token_limit > 0 && hour_used >= self.hour_token_limit {
            return RateLimitResult::Limited {
                retry_after_secs: 1,
            };
        }

        // Check request limit
        if self.minute_req_limit > 0 && self.requests.len() >= self.minute_req_limit as usize {
            let oldest = self.requests.front().copied().unwrap_or(now);
            let secs = oldest
                .duration_since(now)
                .checked_add(window)
                .unwrap_or(window);
            return RateLimitResult::Limited {
                retry_after_secs: secs.as_secs().max(1),
            };
        }

        self.requests.push_back(now);
        RateLimitResult::Allowed {
            remaining: self.minute_token_limit.saturating_sub(minute_used).min(
                self.minute_req_limit
                    .saturating_sub(self.requests.len() as u64),
            ),
        }
    }

    /// Record tokens after response and check limits.
    fn record_tokens(&mut self, tokens: u64) -> RateLimitResult {
        let now = Instant::now();

        // Prune minute tokens outside the 60s sliding window
        while let Some(&(t, _)) = self.minute_tokens.front() {
            if now.duration_since(t) > Duration::from_secs(60) {
                self.minute_tokens.pop_front();
            } else {
                break;
            }
        }
        // Prune hour tokens outside the 3600s sliding window
        while let Some(&(t, _)) = self.hour_tokens.front() {
            if now.duration_since(t) > Duration::from_secs(3600) {
                self.hour_tokens.pop_front();
            } else {
                break;
            }
        }

        let minute_sum: u64 = self.minute_tokens.iter().map(|(_, t)| t).sum();
        let hour_sum: u64 = self.hour_tokens.iter().map(|(_, t)| t).sum();

        if self.hour_token_limit > 0 && hour_sum + tokens > self.hour_token_limit {
            return RateLimitResult::Limited {
                retry_after_secs: 1,
            };
        }
        if self.minute_token_limit > 0 && minute_sum + tokens > self.minute_token_limit {
            return RateLimitResult::Limited {
                retry_after_secs: 1,
            };
        }

        self.minute_tokens.push_back((now, tokens));
        self.hour_tokens.push_back((now, tokens));

        let new_minute_sum = minute_sum + tokens;
        RateLimitResult::Allowed {
            remaining: self.minute_token_limit.saturating_sub(new_minute_sum),
        }
    }
}

/// Thread-safe token/request rate limiter with per-key sliding windows.
pub struct TokenRateLimiter {
    windows: DashMap<String, SlidingWindow>,
    pub config: RateLimitConfig,
}

impl TokenRateLimiter {
    /// Create a new rate limiter with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            windows: DashMap::new(),
            config,
        }
    }

    /// Check before sending request. Returns whether request can proceed.
    ///
    /// When all limits are zero (unlimited), always returns `Allowed`.
    pub fn check(&self, key: &str) -> RateLimitResult {
        if self.config.tokens_per_minute == 0
            && self.config.tokens_per_hour == 0
            && self.config.requests_per_minute == 0
        {
            return RateLimitResult::Allowed {
                remaining: u64::MAX,
            };
        }

        let mut entry = self
            .windows
            .entry(key.to_owned())
            .or_insert_with(|| SlidingWindow::new(&self.config));
        entry.check()
    }

    /// Record token usage after response and check token limits.
    ///
    /// When all limits are zero (unlimited), always returns `Allowed`.
    pub fn record_usage(&self, key: &str, total_tokens: u64) -> RateLimitResult {
        if self.config.tokens_per_minute == 0 && self.config.tokens_per_hour == 0 {
            return RateLimitResult::Allowed {
                remaining: u64::MAX,
            };
        }

        let mut entry = self
            .windows
            .entry(key.to_owned())
            .or_insert_with(|| SlidingWindow::new(&self.config));
        entry.record_tokens(total_tokens)
    }
}
