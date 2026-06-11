use std::sync::Arc;

use ntgw_ai::ab_test::{ABTest, ABTestEngine, Variant};

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
fn test_concurrent_select_variant_no_deadlock() {
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
