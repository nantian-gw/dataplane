use super::*;

#[test]
fn decodes_http_session_persistence_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        http_routes: vec![proto::HttpRoute {
            name: "route".to_string(),
            namespace: "default".to_string(),
            rules: vec![proto::HttpRule {
                name: String::new(),
                session_persistence: Some(proto::SessionPersistence {
                    session_name: "sticky".to_string(),
                    absolute_timeout: Some(prost_types::Duration {
                        seconds: 300,
                        nanos: 0,
                    }),
                    idle_timeout: Some(prost_types::Duration {
                        seconds: 60,
                        nanos: 0,
                    }),
                    r#type: proto::SessionPersistenceType::SessionPersistenceCookie as i32,
                    cookie: Some(proto::CookieConfig {
                        lifetime_type: proto::CookieLifetimeType::CookieLifetimePermanent as i32,
                    }),
                }),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    });

    let policy = snapshot.http_routes[0].rules[0]
        .session_persistence
        .as_ref()
        .expect("session persistence");
    let cookie = policy.cookie.as_ref().expect("cookie config");

    assert_eq!(policy.session_name, "sticky");
    assert_eq!(policy.session_type, "Cookie");
    assert_eq!(
        policy.absolute_timeout,
        Some(std::time::Duration::from_secs(300))
    );
    assert_eq!(
        policy.idle_timeout,
        Some(std::time::Duration::from_secs(60))
    );
    assert_eq!(cookie.lifetime_type, "Permanent");
}

#[test]
fn decodes_backend_session_persistence_from_proto() {
    let snapshot = Snapshot::from(proto::ConfigSnapshot {
        backends: vec![proto::BackendCluster {
            name: "orders:8080".to_string(),
            namespace: "default".to_string(),
            session_persistence: Some(proto::SessionPersistence {
                session_name: "sticky-backend".to_string(),
                r#type: proto::SessionPersistenceType::SessionPersistenceCookie as i32,
                cookie: Some(proto::CookieConfig {
                    lifetime_type: proto::CookieLifetimeType::CookieLifetimeSession as i32,
                }),
                ..Default::default()
            }),
            wasm_plugin: None,
            ai_service: None,
            token_policy: None,
            ..Default::default()
        }],
        ..Default::default()
    });

    let policy = snapshot
        .backend_policy("default/orders:8080")
        .and_then(|policy| policy.session_persistence.as_ref())
        .expect("backend session persistence");
    assert_eq!(policy.session_name, "sticky-backend");
    assert_eq!(policy.session_type, "Cookie");
    assert_eq!(
        policy.cookie.as_ref().expect("cookie config").lifetime_type,
        "Session"
    );
}
