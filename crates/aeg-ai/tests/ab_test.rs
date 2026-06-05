use aeg_ai::ab_test::{ABTest, ABTestEngine, Variant};

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

    let first_three: Vec<String> = (0..3)
        .map(|_| engine.select_variant("det").unwrap().name.clone())
        .collect();

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
        .map(|_| engine2.select_variant("det").unwrap().name.clone())
        .collect();

    assert_eq!(first_three, second_three);
}
