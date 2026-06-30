use super::*;

fn ephemeral_session_persistence_summary_value() -> serde_json::Value {
    let snapshot = Snapshot {
        id: "v-sticky".to_string(),
        http_routes: vec![ntgw_ir::HttpRoute {
            name: "sticky".to_string().into(),
            namespace: "default".to_string().into(),
            rules: vec![HttpRule {
                name: String::new(),
                session_persistence: Some(SessionPersistence {
                    session_name: "sticky".to_string(),
                    session_type: "Cookie".to_string(),
                    cookie: Some(CookieConfig {
                        lifetime_type: "Permanent".to_string(),
                    }),
                    ..SessionPersistence::default()
                }),
                ..HttpRule::default()
            }],
            ..Default::default()
        }],
        backend_policies: std::iter::once((
            "default/api:80".to_string(),
            BackendPolicy {
                session_persistence: Some(SessionPersistence {
                    session_name: "backend-sticky".to_string(),
                    session_type: "Header".to_string(),
                    ..SessionPersistence::default()
                }),
                ..BackendPolicy::default()
            },
        ))
        .collect(),
        ..Snapshot::default()
    };
    let shared = Snapshot::shared();
    shared.store(Arc::new(snapshot));

    let mut config = test_admin_runtime_config();
    config.session_persistence_uses_ephemeral_secret = true;
    let state = build_state_with_parts(
        config,
        shared,
        RuntimeStats::shared(),
        ClientStats::shared(),
    );

    build_summary_value(&state)
}

include!("session_persistence/surface.rs");
include!("session_persistence/warnings.rs");
include!("session_persistence/features.rs");
