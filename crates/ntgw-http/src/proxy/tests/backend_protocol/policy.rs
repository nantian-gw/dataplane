#[test]
fn apply_backend_policy_sets_peer_timeouts() {
    let mut peer = HttpPeer::new(("127.0.0.1", 8080), false, String::new());

    apply_backend_policy(
        &mut peer,
        Some(&BackendPolicy {
            connect_timeout: Some(std::time::Duration::from_secs(3)),
            request_timeout: Some(std::time::Duration::from_secs(15)),
            tls_validation: None,
            session_persistence: None,
            load_balancing: None,
            health_check: None,
            outlier_detection: None,
        }),
        None,
    );

    assert_eq!(
        peer.options.connection_timeout,
        Some(std::time::Duration::from_secs(3))
    );
    assert_eq!(
        peer.options.total_connection_timeout,
        Some(std::time::Duration::from_secs(3))
    );
    assert_eq!(
        peer.options.read_timeout,
        Some(std::time::Duration::from_secs(15))
    );
    assert_eq!(
        peer.options.write_timeout,
        Some(std::time::Duration::from_secs(15))
    );
}

#[test]
fn route_timeouts_override_backend_policy_defaults() {
    let mut peer = HttpPeer::new(("127.0.0.1", 8080), false, String::new());

    apply_backend_policy(
        &mut peer,
        Some(&BackendPolicy {
            connect_timeout: Some(std::time::Duration::from_secs(3)),
            request_timeout: Some(std::time::Duration::from_secs(30)),
            tls_validation: None,
            session_persistence: None,
            load_balancing: None,
            health_check: None,
            outlier_detection: None,
        }),
        Some(&RouteTimeouts {
            request: Some(std::time::Duration::from_secs(12)),
            backend_request: Some(std::time::Duration::from_secs(5)),
            connect: None,
            next_upstream: None,
        }),
    );

    assert_eq!(
        peer.options.read_timeout,
        Some(std::time::Duration::from_secs(5))
    );
    assert_eq!(
        peer.options.write_timeout,
        Some(std::time::Duration::from_secs(5))
    );
}

#[test]
fn zero_backend_policy_request_timeout_is_ignored() {
    let mut peer = HttpPeer::new(("127.0.0.1", 8080), false, String::new());

    apply_backend_policy(
        &mut peer,
        Some(&BackendPolicy {
            connect_timeout: Some(std::time::Duration::from_secs(3)),
            request_timeout: Some(std::time::Duration::ZERO),
            tls_validation: None,
            session_persistence: None,
            load_balancing: None,
            health_check: None,
            outlier_detection: None,
        }),
        None,
    );

    assert_eq!(
        peer.options.connection_timeout,
        Some(std::time::Duration::from_secs(3))
    );
    assert_eq!(peer.options.read_timeout, None);
    assert_eq!(peer.options.write_timeout, None);
}
