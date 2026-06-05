use super::*;

#[test]
fn stream_listener_plan_ignores_non_topology_listener_changes() {
    let base = Listener {
        name: "default/gw/tcp".to_string(),
        address: "127.0.0.1".to_string(),
        addresses: vec!["127.0.0.1".to_string()],
        port: 9000,
        protocol: "LISTENER_PROTOCOL_TCP".to_string(),
        attached_routes: vec!["default/route-a".to_string()],
        ..Listener::default()
    };
    let mut changed = base.clone();
    changed.attached_routes = vec!["default/route-b".to_string()];
    changed
        .metadata
        .insert("nantian.dev/debug".to_string(), "true".to_string());

    let before = build_listener_plan(&aeg_ir::Snapshot {
        listeners: vec![base],
        ..aeg_ir::Snapshot::default()
    });
    let after = build_listener_plan(&aeg_ir::Snapshot {
        listeners: vec![changed],
        ..aeg_ir::Snapshot::default()
    });

    assert_eq!(before, after);
}

#[test]
fn stream_listener_plan_changes_when_bind_changes() {
    let tcp = Listener {
        name: "default/gw/stream".to_string(),
        address: "127.0.0.1".to_string(),
        addresses: vec!["127.0.0.1".to_string()],
        port: 9000,
        protocol: "LISTENER_PROTOCOL_TCP".to_string(),
        metadata: BTreeMap::new(),
        ..Listener::default()
    };
    let mut moved = tcp.clone();
    moved.port = 9443;

    let tcp_plan = build_listener_plan(&aeg_ir::Snapshot {
        listeners: vec![tcp],
        ..aeg_ir::Snapshot::default()
    })
    .expect("tcp plan");
    let moved_plan = build_listener_plan(&aeg_ir::Snapshot {
        listeners: vec![moved],
        ..aeg_ir::Snapshot::default()
    })
    .expect("moved plan");

    assert_eq!(tcp_plan.listeners[0].protocol, StreamProtocol::Tcp);
    assert_ne!(tcp_plan, moved_plan);
}

#[test]
fn build_listener_plan_ignores_tls_passthrough_listeners() {
    let snapshot = aeg_ir::Snapshot {
        listeners: vec![Listener {
            name: "default/gw/tls".to_string(),
            address: "127.0.0.1".to_string(),
            port: 443,
            protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
            ..Listener::default()
        }],
        ..aeg_ir::Snapshot::default()
    };

    let plan = build_listener_plan(&snapshot);

    assert!(
        plan.is_none(),
        "tls passthrough listeners now belong to aeg-shared-tls"
    );
}
