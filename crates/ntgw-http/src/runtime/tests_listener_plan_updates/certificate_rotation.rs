#[test]
fn listener_updates_restart_when_same_secret_ref_certificate_material_rotates() {
    let active = BTreeMap::from([(
        "0.0.0.0:443".to_string(),
        PlannedListener {
            name: "default/gw/https".to_string().into(),
            bind: "0.0.0.0:443".to_string(),
            protocol: ListenerProtocol::Tls(single_tls_material(
                "default/example-cert",
                "CERT-A",
                "KEY-A",
                None,
            )),
        },
    )]);
    let desired_listener = PlannedListener {
        name: "default/gw/https".to_string().into(),
        bind: "0.0.0.0:443".to_string(),
        protocol: ListenerProtocol::Tls(single_tls_material(
            "default/example-cert",
            "CERT-B",
            "KEY-B",
            None,
        )),
    };
    let desired = ListenerPlan {
        listeners: vec![desired_listener.clone()],
    };

    let updates = listener_updates(&active, Some(&desired), &BTreeSet::new());

    assert_eq!(
        updates,
        ListenerUpdatePlan {
            start: vec![desired_listener],
            stop: vec!["0.0.0.0:443".to_string()],
        }
    );
}

#[test]
fn listener_updates_restart_when_secondary_certificate_ref_material_rotates() {
    const SECONDARY_CERT_PEM: &str =
        include_str!("../../../../../testdata/tls/client.crt");
    const SECONDARY_KEY_PEM: &str = include_str!("../../../../../testdata/tls/client.key");

    let initial = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/https".to_string().into(),
            address: "0.0.0.0".to_string(),
            port: 443,
            protocol: "LISTENER_PROTOCOL_HTTPS".to_string().into(),
            tls: Some(TlsConfig {
                enabled: true,
                passthrough: false,
                secret_refs: vec![
                    "default/example-cert".to_string(),
                    "default/backup-cert".to_string(),
                ],
                sni_hosts: vec![],
                min_version: "1.2".to_string(),
                max_version: "1.3".to_string(),
                frontend_validation: None,
            }),
            ..Listener::default()
        }],
        secrets: vec![
            example_secret_material(),
            SecretMaterial {
                namespace: "default".to_string().into(),
                name: "backup-cert".to_string().into(),
                cert_pem: SECONDARY_CERT_PEM.to_string(),
                key_pem: SECONDARY_KEY_PEM.to_string(),
            },
        ],
        ..Snapshot::default()
    };
    let rotated = Snapshot {
        listeners: initial.listeners.clone(),
        secrets: vec![
            example_secret_material(),
            SecretMaterial {
                namespace: "default".to_string().into(),
                name: "backup-cert".to_string().into(),
                cert_pem: VALID_SERVER_CERT_PEM.to_string(),
                key_pem: VALID_SERVER_KEY_PEM.to_string(),
            },
        ],
        ..Snapshot::default()
    };

    let active_plan =
        build_listener_plan(&initial, &RuntimeOptions::default(), None).expect("active plan");
    let desired_plan =
        build_listener_plan(&rotated, &RuntimeOptions::default(), None).expect("desired plan");
    let active = active_plan
        .listeners
        .iter()
        .cloned()
        .map(|listener| (listener.bind.clone(), listener))
        .collect();

    let updates = listener_updates(&active, Some(&desired_plan), &BTreeSet::new());

    assert_eq!(updates.start, desired_plan.listeners);
    assert_eq!(
        updates.stop,
        vec!["0.0.0.0:443".to_string(), "[::]:443".to_string()]
    );
}

#[test]
fn listener_updates_restart_when_frontend_validation_bundle_rotates() {
    let active = BTreeMap::from([(
        "0.0.0.0:443".to_string(),
        PlannedListener {
            name: "default/gw/https".to_string().into(),
            bind: "0.0.0.0:443".to_string(),
            protocol: ListenerProtocol::Tls(single_tls_material(
                "default/example-cert",
                "CERT-A",
                "KEY-A",
                Some("CA-OLD"),
            )),
        },
    )]);
    let desired_listener = PlannedListener {
        name: "default/gw/https".to_string().into(),
        bind: "0.0.0.0:443".to_string(),
        protocol: ListenerProtocol::Tls(single_tls_material(
            "default/example-cert",
            "CERT-A",
            "KEY-A",
            Some("CA-NEW"),
        )),
    };
    let desired = ListenerPlan {
        listeners: vec![desired_listener.clone()],
    };

    let updates = listener_updates(&active, Some(&desired), &BTreeSet::new());

    assert_eq!(
        updates,
        ListenerUpdatePlan {
            start: vec![desired_listener],
            stop: vec!["0.0.0.0:443".to_string()],
        }
    );
}
