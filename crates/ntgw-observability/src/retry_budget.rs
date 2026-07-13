use parking_lot::RwLock;
use std::sync::Arc;

use serde::Serialize;

const RETRY_BUDGET_TOKEN_SCALE: u64 = 1_000;

pub type SharedRetryBudgetController = Arc<RetryBudgetController>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryBudgetOptions {
    pub enabled: bool,
    pub ratio_percent: u32,
    pub burst: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryBudgetSnapshot {
    pub enabled: bool,
    pub ratio_percent: u32,
    pub burst: u32,
    pub available_tokens: u64,
    pub available_milli_tokens: u64,
    pub retryable_requests_observed_total: u64,
    pub retry_allowed_total: u64,
    pub retry_rejected_total: u64,
}

#[derive(Debug, Clone)]
pub struct RetryBudgetController {
    options: RetryBudgetOptions,
    state: Arc<RwLock<RetryBudgetState>>,
}

#[derive(Debug, Default)]
struct RetryBudgetState {
    available_milli_tokens: u64,
    retryable_requests_observed_total: u64,
    retry_allowed_total: u64,
    retry_rejected_total: u64,
}

impl Default for RetryBudgetOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            ratio_percent: 20,
            burst: 16,
        }
    }
}

impl RetryBudgetController {
    pub fn new(options: RetryBudgetOptions) -> Self {
        let available_milli_tokens = if options.enabled {
            retry_budget_capacity(&options)
        } else {
            0
        };

        Self {
            options,
            state: Arc::new(RwLock::new(RetryBudgetState {
                available_milli_tokens,
                ..RetryBudgetState::default()
            })),
        }
    }

    pub fn shared(options: RetryBudgetOptions) -> SharedRetryBudgetController {
        Arc::new(Self::new(options))
    }

    pub fn observe_retryable_request(&self) {
        if !self.options.enabled {
            return;
        }

        let mut state = self.state.write();
        state.retryable_requests_observed_total =
            state.retryable_requests_observed_total.saturating_add(1);
        state.available_milli_tokens = state
            .available_milli_tokens
            .saturating_add(retry_budget_refill(&self.options))
            .min(retry_budget_capacity(&self.options));
    }

    pub fn try_acquire_retry(&self) -> bool {
        if !self.options.enabled {
            return true;
        }

        let mut state = self.state.write();
        if state.available_milli_tokens < RETRY_BUDGET_TOKEN_SCALE {
            state.retry_rejected_total = state.retry_rejected_total.saturating_add(1);
            return false;
        }

        state.available_milli_tokens -= RETRY_BUDGET_TOKEN_SCALE;
        state.retry_allowed_total = state.retry_allowed_total.saturating_add(1);
        true
    }

    pub fn snapshot(&self) -> RetryBudgetSnapshot {
        let state = self.state.read();
        RetryBudgetSnapshot {
            enabled: self.options.enabled,
            ratio_percent: self.options.ratio_percent.min(100),
            burst: self.options.burst.max(1),
            available_tokens: state.available_milli_tokens / RETRY_BUDGET_TOKEN_SCALE,
            available_milli_tokens: state.available_milli_tokens,
            retryable_requests_observed_total: state.retryable_requests_observed_total,
            retry_allowed_total: state.retry_allowed_total,
            retry_rejected_total: state.retry_rejected_total,
        }
    }
}

fn retry_budget_refill(options: &RetryBudgetOptions) -> u64 {
    u64::from(options.ratio_percent.min(100)) * RETRY_BUDGET_TOKEN_SCALE / 100
}

fn retry_budget_capacity(options: &RetryBudgetOptions) -> u64 {
    u64::from(options.burst.max(1)) * RETRY_BUDGET_TOKEN_SCALE
}

#[cfg(test)]
mod tests;
