#[test]
fn skips_http3_listener_when_runtime_has_no_support() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/h3".to_string().into(),
            address: "0.0.0.0".to_string(),
            port: 443,
            protocol: "LISTENER_PROTOCOL_HTTP3".to_string().into(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot, &RuntimeOptions::default(), None);
    assert!(plan.is_none());
    assert!(!http3_available());
}

#[test]
#[cfg(any(target_os = "linux", target_os = "android"))]
fn suppresses_warning_for_ipv6_address_family_not_supported() {
    let err = std::io::Error::from_raw_os_error(97);

    assert!(should_suppress_unavailable_bind_warning("[::]:80", &err));
    assert!(!should_suppress_unavailable_bind_warning(
        "0.0.0.0:80",
        &err
    ));
}

#[test]
fn keeps_warning_for_other_ipv6_bind_failures() {
    let err = std::io::Error::new(std::io::ErrorKind::AddrInUse, "Address already in use");

    assert!(!should_suppress_unavailable_bind_warning("[::]:80", &err));
}

#[test]
fn skips_unbindable_listener_addresses_without_dropping_other_listeners() {
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
                address: "203.0.113.13".to_string(),
                port: 8080,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };

    let plan = build_listener_plan_with_bind_checker(
        &snapshot,
        &RuntimeOptions::default(),
        &Default::default(),
        |bind| {
            if bind == "203.0.113.13:8080" {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrNotAvailable,
                    "Cannot assign requested address",
                ));
            }
            Ok(())
        },
    )
    .expect("plan");

    assert_eq!(plan.listeners.len(), 2);
    assert_eq!(plan.listeners[0].bind, "0.0.0.0:80");
    assert_eq!(plan.listeners[1].bind, "[::]:80");
}

#[test]
fn drops_listener_plan_when_all_addresses_are_unbindable() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http-8080".to_string().into(),
            address: "203.0.113.13".to_string(),
            port: 8080,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
            ..Listener::default()
        }],
        ..Snapshot::default()
    };

    let plan = build_listener_plan_with_bind_checker(
        &snapshot,
        &RuntimeOptions::default(),
        &Default::default(),
        |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                "Cannot assign requested address",
            ))
        },
    );

    assert!(plan.is_none());
}

#[test]
fn keeps_active_listener_binds_without_reprobing_them() {
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
    let active = ListenerPlan {
        listeners: vec![
            PlannedListener {
                name: "default/gw/http".to_string().into(),
                bind: "0.0.0.0:80".to_string(),
                protocol: ListenerProtocol::Plain,
            },
            PlannedListener {
                name: "default/gw/http".to_string().into(),
                bind: "[::]:80".to_string(),
                protocol: ListenerProtocol::Plain,
            },
        ],
    };

    let plan = build_listener_plan_with_bind_checker(
        &snapshot,
        &RuntimeOptions::default(),
        &active
            .listeners
            .iter()
            .map(|listener| listener.bind.clone())
            .collect(),
        |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "Address already in use",
            ))
        },
    )
    .expect("plan");

    assert_eq!(plan, active);
}

#[test]
fn force_reload_uses_active_listener_binds_for_plan_availability() {
    let active = ListenerPlan {
        listeners: vec![
            PlannedListener {
                name: "default/gw/http".to_string().into(),
                bind: "0.0.0.0:80".to_string(),
                protocol: ListenerProtocol::Plain,
            },
            PlannedListener {
                name: "default/gw/http".to_string().into(),
                bind: "[::]:80".to_string(),
                protocol: ListenerProtocol::Plain,
            },
        ],
    };

    let active_binds = active_listener_binds_for_plan_build(Some(&active), true);

    assert_eq!(
        active_binds,
        BTreeSet::from(["0.0.0.0:80".to_string(), "[::]:80".to_string()])
    );
}
