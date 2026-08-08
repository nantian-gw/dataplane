use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
/// using `thread_rng()` for lock-free concurrent access.
pub struct ABTestEngine {
    experiments: HashMap<String, ABTest>,
}

fn select_variant_for_roll(experiment: &ABTest, roll: f64) -> Option<Variant> {
    if experiment.variants.is_empty() {
        return None;
    }

    let mut cumulative = 0.0_f64;
    for variant in &experiment.variants {
        cumulative += variant.weight;
        if roll < cumulative {
            return Some(variant.clone());
        }
    }

    experiment.variants.last().cloned()
}

impl ABTestEngine {
    /// Create a new engine.
    pub fn new() -> Self {
        Self {
            experiments: HashMap::new(),
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
    #[must_use]
    pub fn select_variant(&self, experiment_id: &str) -> Option<Variant> {
        let experiment = self.experiments.get(experiment_id)?;
        let roll: f64 = rand::thread_rng().r#gen();
        select_variant_for_roll(experiment, roll)
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

    #[test]
    fn test_selects_correct_variant_by_weight() {
        let mut engine = ABTestEngine::new();
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
        assert!(counts[0] > 200, "variant a count {} too low", counts[0]);
        assert!(counts[1] > 600, "variant b count {} too low", counts[1]);
    }

    #[test]
    fn test_all_weight_goes_to_first_when_weight_is_1() {
        let mut engine = ABTestEngine::new();
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
        let engine = ABTestEngine::new();
        assert!(engine.select_variant("nonexistent").is_none());
    }

    #[test]
    fn test_select_variant_falls_back_to_last_variant_for_uncovered_roll() {
        let experiment = ABTest {
            experiment_id: "fallback".into(),
            variants: vec![
                Variant {
                    name: "a".into(),
                    model: "gpt-4".into(),
                    weight: 0.1,
                    config: serde_json::Value::Null,
                },
                Variant {
                    name: "b".into(),
                    model: "gpt-4-turbo".into(),
                    weight: 0.2,
                    config: serde_json::Value::Null,
                },
            ],
        };

        let selected = select_variant_for_roll(&experiment, 0.95).unwrap();
        assert_eq!(selected.name, "b");
        assert_eq!(selected.model, "gpt-4-turbo");
    }

    #[test]
    fn test_select_variant_returns_none_for_empty_variants() {
        let experiment = ABTest {
            experiment_id: "empty".into(),
            variants: vec![],
        };

        assert!(select_variant_for_roll(&experiment, 0.5).is_none());
    }

    #[test]
    fn test_concurrent_select_variant_no_deadlock() {
        use std::sync::Arc;

        let mut engine = ABTestEngine::new();
        engine.register(ABTest {
            experiment_id: "concurrent".into(),
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
        let engine = Arc::new(engine);
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let engine = Arc::clone(&engine);
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        let _ = engine.select_variant("concurrent");
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
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
