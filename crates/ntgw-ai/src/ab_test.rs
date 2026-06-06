use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use parking_lot::Mutex;

/// A single variant in an A/B test experiment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Variant {
    /// Human-readable name, e.g. "control", "treatment-a".
    pub name: String,
    /// The model to route to when this variant is selected.
    pub model: String,
    /// Probability weight (0.0–1.0). The sum across all variants should be 1.0.
    pub weight: f64,
    /// Arbitrary configuration attached to the variant.
    #[serde(default)]
    pub config: serde_json::Value,
}

/// An A/B test experiment containing a set of weighted variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTest {
    /// Unique identifier for this experiment.
    pub experiment_id: String,
    /// Weighted variants; the first variant whose cumulative weight exceeds the
    /// random roll is selected.
    pub variants: Vec<Variant>,
}

/// Weighted-random selection engine for A/B test experiments.
///
/// Stores experiments by ID and provides weighted-random variant selection
/// using a seeded PRNG for reproducibility when desired.
pub struct ABTestEngine {
    experiments: HashMap<String, ABTest>,
    rng: Mutex<StdRng>,
}

impl ABTestEngine {
    /// Create a new engine with a randomly-seeded PRNG.
    pub fn new() -> Self {
        Self {
            experiments: HashMap::new(),
            rng: Mutex::new(StdRng::from_entropy()),
        }
    }

    /// Create a new engine with a fixed PRNG seed (useful for deterministic tests).
    pub fn with_seed(seed: u64) -> Self {
        Self {
            experiments: HashMap::new(),
            rng: Mutex::new(StdRng::seed_from_u64(seed)),
        }
    }

    /// Register an experiment, replacing any existing experiment with the same ID.
    pub fn register(&mut self, experiment: ABTest) {
        self.experiments
            .insert(experiment.experiment_id.clone(), experiment);
    }

    /// Select a variant for the given experiment using weighted random.
    ///
    /// Returns `None` if the experiment is not found. When weights do not sum to
    /// 1.0 the last variant whose cumulative weight exceeds the random roll is
    /// returned. If all weights are zero the first variant is returned.
    pub fn select_variant(&self, experiment_id: &str) -> Option<Variant> {
        let experiment = self.experiments.get(experiment_id)?;
        if experiment.variants.is_empty() {
            return None;
        }

        let mut rng = self.rng.lock();
        let roll: f64 = rng.gen();

        let mut cumulative = 0.0_f64;
        for variant in &experiment.variants {
            cumulative += variant.weight;
            if roll < cumulative {
                return Some(variant.clone());
            }
        }

        // Fallback: return last variant if roll >= total weight
        // (can happen when weights sum < 1.0)
        #[allow(clippy::unwrap_used)]
        Some(experiment.variants.last().expect("ab-test has non-empty variants").clone())
    }

    /// Generate a unique experiment identifier.
    pub fn generate_id() -> String {
        format!("exp_{}", uuid::Uuid::new_v4().to_string().replace('-', ""))
    }

    /// Return the number of registered experiments.
    pub fn len(&self) -> usize {
        self.experiments.len()
    }

    /// Return whether the engine has any experiments.
    pub fn is_empty(&self) -> bool {
        self.experiments.is_empty()
    }
}

impl Default for ABTestEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── tests within this module measure just the core selection logic ──

    #[test]
    fn test_selects_correct_variant_by_weight() {
        let mut engine = ABTestEngine::with_seed(42);
        engine.register(ABTest {
            experiment_id: "test_exp".into(),
            variants: vec![
                Variant {
                    name: "a".into(),
                    model: "gpt-4".into(),
                    weight: 0.3,
                    config: serde_json::Value::Null,
                },
                Variant {
                    name: "b".into(),
                    model: "gpt-4-turbo".into(),
                    weight: 0.7,
                    config: serde_json::Value::Null,
                },
            ],
        });

        let mut counts = [0usize; 2];
        for _ in 0..1000 {
            #[allow(clippy::unwrap_used)]
            let v = engine.select_variant("test_exp").unwrap();
            if v.name == "a" {
                counts[0] += 1;
            } else {
                counts[1] += 1;
            }
        }
        // With 1000 trials and weights 0.3/0.7, counts should be roughly
        // 300/700. We assert a generous tolerance to avoid flaky failures.
        assert!(counts[0] > 200, "variant a count {} too low", counts[0]);
        assert!(counts[1] > 600, "variant b count {} too low", counts[1]);
    }

    #[test]
    fn test_all_weight_goes_to_first_when_weight_is_1() {
        let mut engine = ABTestEngine::with_seed(99);
        engine.register(ABTest {
            experiment_id: "single".into(),
            variants: vec![Variant {
                name: "only".into(),
                model: "gemini-pro".into(),
                weight: 1.0,
                config: serde_json::Value::Null,
            }],
        });

        for _ in 0..50 {
            #[allow(clippy::unwrap_used)]
            let v = engine.select_variant("single").unwrap();
            assert_eq!(v.name, "only");
            assert_eq!(v.model, "gemini-pro");
        }
    }

    #[test]
    fn test_returns_none_for_unknown_experiment() {
        let engine = ABTestEngine::with_seed(1);
        assert!(engine.select_variant("nonexistent").is_none());
    }

    #[test]
    fn test_deterministic_with_fixed_seed() {
        let mut engine = ABTestEngine::with_seed(7);
        engine.register(ABTest {
            experiment_id: "det".into(),
            variants: vec![
                Variant {
                    name: "x".into(),
                    model: "m-a".into(),
                    weight: 0.5,
                    config: serde_json::Value::Null,
                },
                Variant {
                    name: "y".into(),
                    model: "m-b".into(),
                    weight: 0.5,
                    config: serde_json::Value::Null,
                },
            ],
        });

        // With fixed seed the sequence is deterministic
        let first_three: Vec<String> = (0..3)
            .map(|_| {
                #[allow(clippy::unwrap_used)]
                engine.select_variant("det").unwrap().name.clone()
            })
            .collect();

        // Same seed → same sequence
        let mut engine2 = ABTestEngine::with_seed(7);
        engine2.register(ABTest {
            experiment_id: "det".into(),
            variants: vec![
                Variant {
                    name: "x".into(),
                    model: "m-a".into(),
                    weight: 0.5,
                    config: serde_json::Value::Null,
                },
                Variant {
                    name: "y".into(),
                    model: "m-b".into(),
                    weight: 0.5,
                    config: serde_json::Value::Null,
                },
            ],
        });

        let second_three: Vec<String> = (0..3)
            .map(|_| {
                #[allow(clippy::unwrap_used)]
                engine2.select_variant("det").unwrap().name.clone()
            })
            .collect();

        assert_eq!(first_three, second_three);
    }

    #[test]
    fn test_generate_id_is_unique() {
        let id1 = ABTestEngine::generate_id();
        let id2 = ABTestEngine::generate_id();
        assert_ne!(id1, id2);
        assert!(id1.starts_with("exp_"));
        assert!(id2.starts_with("exp_"));
    }

    #[test]
    fn test_empty_engine_is_empty() {
        let engine = ABTestEngine::new();
        assert!(engine.is_empty());
        assert_eq!(engine.len(), 0);
    }
}
