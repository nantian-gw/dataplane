#[test]
fn hostname_intersection_conformance_case_does_not_leak_to_unspecified_listener() {
    let snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "gateway-conformance-infra/httproute-hostname-intersection/listener-1"
                    .to_string(),
                address: "0.0.0.0".to_string(),
                addresses: vec!["0.0.0.0".to_string()],
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                hostnames: vec!["very.specific.com".to_string()],
                attached_routes: vec![
                    "gateway-conformance-infra/specific-host-matches-listener-specific-host"
                        .to_string(),
                    "gateway-conformance-infra/wildcard-host-matches-listener-specific-host"
                        .to_string(),
                ],
                tls: None,
                backend_tls: None,
                metadata: BTreeMap::new(),
            },
            Listener {
                name: "gateway-conformance-infra/httproute-hostname-intersection/listener-2"
                    .to_string(),
                address: "0.0.0.0".to_string(),
                addresses: vec!["0.0.0.0".to_string()],
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                hostnames: vec!["*.wildcard.io".to_string()],
                attached_routes: vec![
                    "gateway-conformance-infra/specific-host-matches-listener-wildcard-host"
                        .to_string(),
                ],
                tls: None,
                backend_tls: None,
                metadata: BTreeMap::new(),
            },
            Listener {
                name: "gateway-conformance-infra/httproute-hostname-intersection/listener-3"
                    .to_string(),
                address: "0.0.0.0".to_string(),
                addresses: vec!["0.0.0.0".to_string()],
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                hostnames: vec!["*.anotherwildcard.io".to_string()],
                attached_routes: vec![
                    "gateway-conformance-infra/wildcard-host-matches-listener-wildcard-host"
                        .to_string(),
                ],
                tls: None,
                backend_tls: None,
                metadata: BTreeMap::new(),
            },
            Listener {
                name: "gateway-conformance-infra/httproute-hostname-intersection-all/listener-1"
                    .to_string(),
                address: "0.0.0.0".to_string(),
                addresses: vec!["0.0.0.0".to_string()],
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
                hostnames: vec![],
                attached_routes: vec![
                    "gateway-conformance-infra/httproute-hostname-intersection-all".to_string(),
                ],
                tls: None,
                backend_tls: None,
                metadata: BTreeMap::new(),
            },
        ],
        http_routes: vec![
            HttpRoute {
                name: "wildcard-host-matches-listener-wildcard-host".to_string(),
                namespace: "gateway-conformance-infra".to_string(),
                hostnames: vec!["*.anotherwildcard.io".to_string()],
                parent_refs: vec![],
                rules: vec![path_rule(
                    "/s4",
                    "gateway-conformance-infra",
                    "infra-backend-v1",
                    8080,
                )],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
            HttpRoute {
                name: "httproute-hostname-intersection-all".to_string(),
                namespace: "gateway-conformance-infra".to_string(),
                hostnames: vec![
                    "first.com".to_string(),
                    "sub.first.com".to_string(),
                    "second.com".to_string(),
                    "sub.second.com".to_string(),
                ],
                parent_refs: vec![],
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![],
                    filters: vec![],
                    backend_refs: vec![backend_ref(
                        "gateway-conformance-infra",
                        "infra-backend-v2",
                        8080,
                    )],
                    timeouts: None,
                    retry: None,
                    session_persistence: None,
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            },
        ],
        backends: vec![
            backend_cluster("gateway-conformance-infra", "infra-backend-v1", "10.0.0.1"),
            backend_cluster("gateway-conformance-infra", "infra-backend-v2", "10.0.0.2"),
        ],
        ..Snapshot::default()
    };

    let matched = snapshot.select_backend(&RequestMeta::with_port(
        Some("foo.anotherwildcard.io".to_string()),
        80,
        "/s4",
        "GET",
        BTreeMap::new(),
    ));
    assert_eq!(
        matched.expect("wildcard listener backend").backend.address,
        "10.0.0.1"
    );

    let blocked = snapshot.select_backend(&RequestMeta::with_port(
        Some("anotherwildcard.io".to_string()),
        80,
        "/s4",
        "GET",
        BTreeMap::new(),
    ));
    assert!(blocked.is_none());

    let unspecified = snapshot.select_backend(&RequestMeta::with_port(
        Some("first.com".to_string()),
        80,
        "/s4",
        "GET",
        BTreeMap::new(),
    ));
    assert_eq!(
        unspecified
            .expect("unspecified listener backend")
            .backend
            .address,
        "10.0.0.2"
    );
}
