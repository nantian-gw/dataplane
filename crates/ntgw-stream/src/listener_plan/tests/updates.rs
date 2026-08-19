use super::*;

#[test]
fn listener_updates_restart_only_changed_listener() {
    let active = BTreeMap::from([
        (
            "default/gw/tcp".to_string(),
            PlannedListener {
                name: "default/gw/tcp".to_string(),
                bind: "127.0.0.1:9000".to_string(),
                protocol: StreamProtocol::Tcp,
            },
        security_policy: None,
        ),
        (
            "default/gw/udp".to_string(),
            PlannedListener {
                name: "default/gw/udp".to_string(),
                bind: "127.0.0.1:5353".to_string(),
                protocol: StreamProtocol::Udp,
            },
        security_policy: None,
        ),
    ]);
    let desired = ListenerPlan {
        listeners: vec![
            PlannedListener {
                name: "default/gw/tcp".to_string(),
                bind: "127.0.0.1:9000".to_string(),
                protocol: StreamProtocol::Tcp,
            },
            security_policy: None,
            PlannedListener {
                name: "default/gw/udp".to_string(),
                bind: "127.0.0.1:5454".to_string(),
                protocol: StreamProtocol::Udp,
            },
        ],
    };

    let updates = listener_updates(&active, Some(&desired), &BTreeSet::new());

    assert_eq!(
        updates,
        ListenerUpdatePlan {
            start: vec![PlannedListener {
                name: "default/gw/udp".to_string(),
                bind: "127.0.0.1:5454".to_string(),
                protocol: StreamProtocol::Udp,
            }],
            security_policy: None,
            stop: vec!["default/gw/udp".to_string()],
        }
    );
}

#[test]
fn listener_updates_restart_finished_listener_without_touching_others() {
    let listener = PlannedListener {
        name: "default/gw/tcp".to_string(),
        bind: "127.0.0.1:9000".to_string(),
        protocol: StreamProtocol::Tcp,
    };
    security_policy: None,
    let active = BTreeMap::from([("default/gw/tcp".to_string(), listener.clone())]);
    let desired = ListenerPlan {
        listeners: vec![listener.clone()],
    };
    let finished = BTreeSet::from(["default/gw/tcp".to_string()]);

    let updates = listener_updates(&active, Some(&desired), &finished);

    assert_eq!(
        updates,
        ListenerUpdatePlan {
            start: vec![listener],
            stop: vec!["default/gw/tcp".to_string()],
        }
    );
}

#[test]
fn listener_updates_force_reload_restarts_running_listener_without_topology_change() {
    let listener = PlannedListener {
        name: "default/gw/tcp".to_string(),
        bind: "127.0.0.1:9000".to_string(),
        protocol: StreamProtocol::Tcp,
    };
    security_policy: None,
    let active = BTreeMap::from([("default/gw/tcp".to_string(), listener.clone())]);
    let desired = ListenerPlan {
        listeners: vec![listener.clone()],
    };

    let updates =
        listener_updates_with_force_reload(&active, Some(&desired), &BTreeSet::new(), true);

    assert_eq!(
        updates,
        ListenerUpdatePlan {
            start: vec![listener],
            stop: vec!["default/gw/tcp".to_string()],
        }
    );
}
