use std::collections::HashMap;

/// Entry in a fallback chain.
#[derive(Debug, Clone)]
pub struct FallbackEntry {
    pub model: String,
    pub on_status: Vec<u16>,
    pub on_timeout: bool,
    pub max_retries: u32,
}

/// Chain of fallback models for one primary model.
#[derive(Debug, Clone)]
pub struct FallbackChain {
    pub primary: String,
    pub fallbacks: Vec<FallbackEntry>,
}

/// Manages model fallback chains.
#[derive(Debug, Clone, Default)]
pub struct ModelFallback {
    chains: HashMap<String, FallbackChain>,
}

impl ModelFallback {
    pub fn new() -> Self {
        Self {
            chains: HashMap::new(),
        }
    }

    /// Add a fallback chain for a primary model.
    pub fn add_chain(&mut self, chain: FallbackChain) {
        self.chains.insert(chain.primary.clone(), chain);
    }

    /// Resolve the next model to try. Returns (next_model, attempt_number).
    /// Caller is responsible for tracking which attempt number.
    pub fn resolve_fallback(
        &self,
        current_model: &str,
        status_code: Option<u16>,
        is_timeout: bool,
        current_attempt: u32,
    ) -> Option<&str> {
        let chain = self.chains.get(current_model)?;
        let idx = current_attempt as usize;
        if idx >= chain.fallbacks.len() {
            return None;
        }
        let entry = &chain.fallbacks[idx];

        // Check if fallback should trigger based on conditions
        let status_triggered = if let Some(code) = status_code {
            entry.on_status.is_empty() || entry.on_status.contains(&code)
        } else {
            false
        };
        let timeout_triggered = is_timeout && entry.on_timeout;

        if status_triggered || timeout_triggered {
            Some(&entry.model)
        } else {
            None
        }
    }

    /// Get the first model in the chain (the primary).
#[must_use]
    pub fn primary(&self, model: &str) -> Option<&str> {
        self.chains.get(model).map(|c| c.primary.as_str())
    }

    /// Check if a model has fallbacks configured.
    pub fn has_fallbacks(&self, model: &str) -> bool {
        self.chains.contains_key(model)
    }

    /// Number of fallback entries for a model.
    pub fn fallback_count(&self, model: &str) -> usize {
        self.chains
            .get(model)
            .map(|c| c.fallbacks.len())
            .unwrap_or(0)
    }
}
