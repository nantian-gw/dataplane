#[test]
fn falls_back_to_default_listener_when_snapshot_has_no_l7_listeners() {
    let plan = build_listener_plan(
        &Snapshot::default(),
        &RuntimeOptions {
            default_listen_addr: "0.0.0.0:80".to_string(),
            enable_ipv6: true,
            ..RuntimeOptions::default()
        },
        None,
    )
    .expect("plan");

    assert_eq!(plan.listeners.len(), 2);
    assert_eq!(plan.listeners[0].bind, "0.0.0.0:80");
    assert_eq!(plan.listeners[1].bind, "[::]:80");
    assert!(matches!(
        plan.listeners[0].protocol,
        ListenerProtocol::Plain
    ));
    assert!(matches!(
        plan.listeners[1].protocol,
        ListenerProtocol::Plain
    ));
}

#[test]
fn uses_runtime_bind_address_for_plain_http_listeners() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string().into(),
            address: "0.0.0.0".to_string(),
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(
        &snapshot,
        &RuntimeOptions {
            default_listen_addr: "0.0.0.0:80".to_string(),
            enable_ipv6: true,
            ..RuntimeOptions::default()
        },
        None,
    )
    .expect("plan");

    assert_eq!(plan.listeners.len(), 2);
    assert_eq!(plan.listeners[0].bind, "0.0.0.0:80");
    assert_eq!(plan.listeners[1].bind, "[::]:80");
    assert!(matches!(
        plan.listeners[0].protocol,
        ListenerProtocol::Plain
    ));
    assert!(matches!(
        plan.listeners[1].protocol,
        ListenerProtocol::Plain
    ));
}

#[test]
fn plain_http_server_options_enable_h2c() {
    let options = plain_http_server_options(None);

    assert!(options.h2c);
}

#[test]
fn plain_http_server_options_apply_keepalive_request_limit() {
    let options = plain_http_server_options(Some(7));

    assert!(options.h2c);
    assert_eq!(options.keepalive_request_limit, Some(7));
}

#[test]
fn binds_all_listener_addresses_from_field() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string().into(),
            address: "192.0.2.10".to_string(),
            addresses: vec!["192.0.2.10".to_string(), "2001:db8::10".to_string()],
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(
        &snapshot,
        &RuntimeOptions {
            enable_ipv6: true,
            ..RuntimeOptions::default()
        },
        None,
    )
    .expect("plan");

    assert_eq!(plan.listeners.len(), 2);
    assert_eq!(plan.listeners[0].bind, "192.0.2.10:80");
    assert_eq!(plan.listeners[1].bind, "[2001:db8::10]:80");
}

#[test]
fn falls_back_to_listener_addresses_metadata() {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        LISTENER_ADDRESSES_METADATA_KEY.to_string(),
        "192.0.2.10,2001:db8::10".to_string(),
    );

    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string().into(),
            address: "192.0.2.10".to_string(),
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
            metadata,
            ..Listener::default()
        }],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(
        &snapshot,
        &RuntimeOptions {
            enable_ipv6: true,
            ..RuntimeOptions::default()
        },
        None,
    )
    .expect("plan");

    assert_eq!(plan.listeners.len(), 2);
    assert_eq!(plan.listeners[0].bind, "192.0.2.10:80");
    assert_eq!(plan.listeners[1].bind, "[2001:db8::10]:80");
}

#[test]
fn preserves_distinct_plain_http_listener_ports() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/http".to_string().into(),
                address: "0.0.0.0".to_string(),
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/http-8080".to_string().into(),
                address: "0.0.0.0".to_string(),
                port: 8080,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(
        &snapshot,
        &RuntimeOptions {
            default_listen_addr: "0.0.0.0:80".to_string(),
            enable_ipv6: true,
            ..RuntimeOptions::default()
        },
        None,
    )
    .expect("plan");

    assert_eq!(plan.listeners.len(), 4);
    assert_eq!(plan.listeners[0].bind, "0.0.0.0:80");
    assert_eq!(plan.listeners[1].bind, "0.0.0.0:8080");
    assert_eq!(plan.listeners[2].bind, "[::]:80");
    assert_eq!(plan.listeners[3].bind, "[::]:8080");
}

#[test]
fn listener_port_hint_uses_common_dual_stack_bind_port() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string().into(),
            address: "0.0.0.0".to_string(),
            port: 80,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };
    let plan = build_listener_plan(
        &snapshot,
        &RuntimeOptions {
            enable_ipv6: true,
            ..RuntimeOptions::default()
        },
        None,
    )
    .expect("plan");
    let runtime_plan = materialize_runtime_plan(&plan, std::path::Path::new("/tmp"));
    let listeners = runtime_plan.listeners.iter().collect::<Vec<_>>();

    assert_eq!(listener_port_hint(&listeners), Some(80));
}

#[test]
fn listener_port_hint_is_absent_for_mixed_listener_ports() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/http".to_string().into(),
                address: "0.0.0.0".to_string(),
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/http-alt".to_string().into(),
                address: "0.0.0.0".to_string(),
                port: 8080,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    let plan = build_listener_plan(
        &snapshot,
        &RuntimeOptions {
            enable_ipv6: true,
            ..RuntimeOptions::default()
        },
        None,
    )
    .expect("plan");
    let runtime_plan = materialize_runtime_plan(&plan, std::path::Path::new("/tmp"));
    let listeners = runtime_plan.listeners.iter().collect::<Vec<_>>();

    assert_eq!(listener_port_hint(&listeners), None);
}

#[test]
fn renders_ipv6_bind_address() {
    assert_eq!(bind_addrs("::", 443, true), vec!["[::]:443".to_string()]);
}

#[test]
fn renders_dual_stack_wildcard_bind_addresses() {
    assert_eq!(
        bind_addrs("0.0.0.0", 443, true),
        vec!["0.0.0.0:443".to_string(), "[::]:443".to_string()]
    );
}

#[test]
fn marks_ipv6_binds_as_ipv6_only() {
    assert_eq!(
        tcp_socket_options_for_bind("[::]:443", None, None, None, None).ipv6_only,
        Some(true)
    );
    assert_eq!(
        tcp_socket_options_for_bind("0.0.0.0:443", None, None, None, None).ipv6_only,
        None
    );
}

#[test]
fn tcp_socket_options_include_configured_keepalive() {
    let keepalive = pingora::protocols::l4::ext::TcpKeepalive {
        idle: std::time::Duration::from_secs(60),
        interval: std::time::Duration::from_secs(15),
        count: 4,
        #[cfg(target_os = "linux")]
        user_timeout: std::time::Duration::from_secs(5),
    };

    let options = tcp_socket_options_for_bind("0.0.0.0:443", Some(keepalive.clone()), Some(true), None, None);

    assert_eq!(options.ipv6_only, None);
    assert_eq!(options.so_reuseport, Some(true));
    let configured = options
        .tcp_keepalive
        .as_ref()
        .expect("configured keepalive");
    assert_eq!(configured.idle, keepalive.idle);
    assert_eq!(configured.interval, keepalive.interval);
    assert_eq!(configured.count, keepalive.count);
    #[cfg(target_os = "linux")]
    assert_eq!(configured.user_timeout, keepalive.user_timeout);
}
