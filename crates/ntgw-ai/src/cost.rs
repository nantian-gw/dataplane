use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::format::ir::AIUsage;

/// Pricing per 1000 tokens for a model.
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_per_1k: f64,
    pub output_per_1k: f64,
}

impl ModelPricing {
    pub fn new(input_per_1k: f64, output_per_1k: f64) -> Self {
        Self {
            input_per_1k,
            output_per_1k,
        }
    }
}

/// Real-time cost tracker. Accumulates cost in deci-cent-dollars (1 unit = 0.0001 dollars)
/// to avoid f64-to-u64 truncation. All public cost values returned are in dollars.
pub struct CostTracker {
    pricing: HashMap<String, ModelPricing>,
    total_cost: AtomicU64,
}

impl CostTracker {
    /// Create a new `CostTracker` with built-in pricing for common models.
    pub fn new() -> Self {
        let mut pricing = HashMap::new();
        // Built-in pricing for common models (per 1K tokens)
        pricing.insert("gpt-4o".into(), ModelPricing::new(2.50, 10.00));
        pricing.insert("gpt-4o-mini".into(), ModelPricing::new(0.15, 0.60));
        pricing.insert("gpt-3.5-turbo".into(), ModelPricing::new(0.50, 1.50));
        pricing.insert("claude-3-opus".into(), ModelPricing::new(15.00, 75.00));
        pricing.insert("claude-3-sonnet".into(), ModelPricing::new(3.00, 15.00));
        pricing.insert("claude-3-haiku".into(), ModelPricing::new(0.25, 1.25));
        Self {
            pricing,
            total_cost: AtomicU64::new(0),
        }
    }

    /// Create a `CostTracker` with custom pricing.
    pub fn with_pricing(pricing: HashMap<String, ModelPricing>) -> Self {
        Self {
            pricing,
            total_cost: AtomicU64::new(0),
        }
    }

    /// Calculate cost for a request (in dollars).
    pub fn calc_cost(&self, model: &str, usage: &AIUsage) -> f64 {
        let pricing = self.pricing.get(model);
        let (input_rate, output_rate) = match pricing {
            Some(p) => (p.input_per_1k, p.output_per_1k),
            None => return 0.0,
        };

        let input_cost = (usage.prompt_tokens as f64 / 1000.0) * input_rate;
        let output_cost = (usage.completion_tokens as f64 / 1000.0) * output_rate;

        (input_cost + output_cost).max(0.0)
    }

    /// Record usage and return cost in dollars.
    pub fn record(&self, model: &str, usage: &AIUsage) -> f64 {
        let cost = self.calc_cost(model, usage);
        let units = (cost * 10_000.0).round() as u64;
        self.total_cost.fetch_add(units, Ordering::Relaxed);
        cost
    }

    /// Total accumulated cost in dollars.
    pub fn total_cost_dollars(&self) -> f64 {
        let units = self.total_cost.load(Ordering::Relaxed);
        units as f64 / 10_000.0
    }

    /// Reset total cost to zero.
    pub fn reset(&self) {
        self.total_cost.store(0, Ordering::Relaxed);
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}
