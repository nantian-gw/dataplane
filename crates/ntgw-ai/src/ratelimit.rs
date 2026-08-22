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
/// Token totals are maintained incrementally so checks do not rescan the full
/// window on every request.
struct SlidingWindow {
    minute_req_limit: u64,
    minute_token_limit: u64,
    hour_token_limit: u64,
    requests: VecDeque<Instant>,
    minute_tokens: VecDeque<(Instant, u64)>,
    hour_tokens: VecDeque<(Instant, u64)>,
    minute_token_total: u64,
    hour_token_total: u64,
}

fn min_enabled_remaining(values: impl IntoIterator<Item = Option<u64>>) -> u64 {
    values.into_iter().flatten().min().unwrap_or(u64::MAX)
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
            minute_token_total: 0,
            hour_token_total: 0,
        }
    }

    fn prune(&mut self, now: Instant) {
        let minute_window = Duration::from_secs(60);

        while let Some(&t) = self.requests.front() {
            if now.saturating_duration_since(t) > minute_window {
                self.requests.pop_front();
            } else {
                break;
            }
        }

        Self::prune_token_window(
            &mut self.minute_tokens,
            &mut self.minute_token_total,
            now,
            minute_window,
        );
        Self::prune_token_window(
            &mut self.hour_tokens,
            &mut self.hour_token_total,
            now,
            Duration::from_secs(3600),
        );
    }

    fn prune_token_window(
        tokens: &mut VecDeque<(Instant, u64)>,
        total: &mut u64,
        now: Instant,
        window: Duration,
    ) {
        while let Some(&(t, _)) = tokens.front() {
            if now.saturating_duration_since(t) > window {
                let Some((_, expired_tokens)) = tokens.pop_front() else {
                    break;
                };
                *total = total.saturating_sub(expired_tokens);
            } else {
                break;
            }
        }
    }

    /// Check before sending request. Returns whether request can proceed.
    /// Also checks token budget from previous requests so that requests
    /// are rejected before incurring upstream cost when the budget is exhausted.
    fn check(&mut self) -> RateLimitResult {
        let now = Instant::now();
        self.prune(now);

        // Check minute token budget from previous requests
        let minute_used = self.minute_token_total;
        if self.minute_token_limit > 0 && minute_used >= self.minute_token_limit {
            return RateLimitResult::Limited {
                retry_after_secs: 1,
            };
        }
        // Check hour token budget from previous requests
        let hour_used = self.hour_token_total;
        if self.hour_token_limit > 0 && hour_used >= self.hour_token_limit {
            return RateLimitResult::Limited {
                retry_after_secs: 1,
            };
        }

        // Check request limit
        if self.minute_req_limit > 0 && self.requests.len() >= self.minute_req_limit as usize {
            let oldest = self.requests.front().copied().unwrap_or(now);
            let secs =
                Duration::from_secs(60).saturating_sub(now.saturating_duration_since(oldest));
            return RateLimitResult::Limited {
                retry_after_secs: secs.as_secs().max(1),
            };
        }

        self.requests.push_back(now);
        RateLimitResult::Allowed {
            remaining: min_enabled_remaining([
                (self.minute_token_limit > 0)
                    .then(|| self.minute_token_limit.saturating_sub(minute_used)),
                (self.hour_token_limit > 0)
                    .then(|| self.hour_token_limit.saturating_sub(hour_used)),
                (self.minute_req_limit > 0).then(|| {
                    self.minute_req_limit
                        .saturating_sub(self.requests.len() as u64)
                }),
            ]),
        }
    }

    /// Record tokens after response and check limits.
    fn record_tokens(&mut self, tokens: u64) -> RateLimitResult {
        let now = Instant::now();
        self.prune(now);

        let projected_minute_total = self.minute_token_total.saturating_add(tokens);
        let projected_hour_total = self.hour_token_total.saturating_add(tokens);

        if self.hour_token_limit > 0 && projected_hour_total > self.hour_token_limit {
            return RateLimitResult::Limited {
                retry_after_secs: 1,
            };
        }
        if self.minute_token_limit > 0 && projected_minute_total > self.minute_token_limit {
            return RateLimitResult::Limited {
                retry_after_secs: 1,
            };
        }

        if self.minute_token_limit > 0 {
            self.minute_tokens.push_back((now, tokens));
            self.minute_token_total = projected_minute_total;
        }
        if self.hour_token_limit > 0 {
            self.hour_tokens.push_back((now, tokens));
            self.hour_token_total = projected_hour_total;
        }

        RateLimitResult::Allowed {
            remaining: min_enabled_remaining([
                (self.minute_token_limit > 0).then(|| {
                    self.minute_token_limit
                        .saturating_sub(self.minute_token_total)
                }),
                (self.hour_token_limit > 0)
                    .then(|| self.hour_token_limit.saturating_sub(self.hour_token_total)),
            ]),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_tokens_updates_running_totals() {
        let config = RateLimitConfig {
            tokens_per_minute: 100,
            tokens_per_hour: 1_000,
            ..Default::default()
        };
        let mut window = SlidingWindow::new(&config);
        let now = Instant::now();

        window
            .minute_tokens
            .push_back((now - Duration::from_secs(61), 40));
        window.minute_tokens.push_back((now, 25));
        window.minute_token_total = 65;

        window
            .hour_tokens
            .push_back((now - Duration::from_secs(3601), 400));
        window.hour_tokens.push_back((now, 125));
        window.hour_token_total = 525;

        window.prune(now);

        assert_eq!(window.minute_token_total, 25);
        assert_eq!(window.hour_token_total, 125);
    }
}
