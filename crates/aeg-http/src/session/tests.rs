use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use pingora::http::{RequestHeader, ResponseHeader};
use proptest::{prelude::*, string::string_regex};
use sha2::Digest;

use super::{
    cookie_path, longest_literal_cookie_path, FileSecretSource, SecretSource, SessionManager,
    SessionPersistenceOptions,
};
use aeg_ir::{BackendEndpoint, MatchedHttpPath, SelectedBackend, SessionPersistence};

fn session_manager() -> SessionManager {
    SessionManager::new(
        SessionPersistenceOptions::build(Some(b"0123456789abcdef0123456789abcdef".to_vec()), None)
            .expect("options"),
    )
}

fn selected_backend() -> SelectedBackend {
    SelectedBackend {
        route_kind: aeg_ir::RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: BTreeMap::new(),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
        backend: BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/echo:8080".to_string(),
        filters: Vec::new(),
        matched_http_path: Some(MatchedHttpPath {
            path: "/app".to_string(),
            path_type: "PathPrefix".to_string(),
        }),
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
    }
}

fn cookie_policy() -> SessionPersistence {
    SessionPersistence {
        session_name: "aeg-http-session".to_string(),
        session_type: "Cookie".to_string(),
        absolute_timeout: Some(Duration::from_secs(300)),
        idle_timeout: Some(Duration::from_secs(60)),
        cookie: Some(aeg_ir::CookieConfig {
            lifetime_type: "Permanent".to_string(),
        }),
    }
}

fn temp_secret_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("aeg-http-{prefix}-{unique}.key"))
}

fn session_name_strategy() -> BoxedStrategy<String> {
    string_regex("[a-z][a-z0-9-]{2,12}")
        .expect("session name regex")
        .boxed()
}

fn backend_name_strategy() -> BoxedStrategy<String> {
    string_regex("default/[a-z][a-z0-9-]{1,10}:[0-9]{2,5}")
        .expect("backend name regex")
        .boxed()
}

include!("tests/options.rs");
include!("tests/cookie_transport.rs");
include!("tests/secret_file.rs");
include!("tests/generated_tokens.rs");
