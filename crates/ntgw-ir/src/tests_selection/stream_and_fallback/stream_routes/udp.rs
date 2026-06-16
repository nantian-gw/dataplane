#[test]
fn selects_udp_backend_by_listener() {
    let snapshot = Snapshot {
        listeners: vec![Listener {
            name: "default/gw/udp".to_string(),
            address: "0.0.0.0".to_string(),
            addresses: vec!["0.0.0.0".to_string()],
            port: 53,
            protocol: "LISTENER_PROTOCOL_UDP".to_string(),
            hostnames: vec![],
            attached_routes: vec!["default/dns".to_string()],
            tls: None,
            backend_tls: None,
            metadata: BTreeMap::new(),
        }],
        stream_routes: vec![StreamRoute {
            name: "dns".to_string(),
            namespace: "default".to_string(),
            kind: "ROUTE_KIND_UDP".to_string(),
            parent_refs: vec![],
            rules: vec![StreamRule {
                name: String::new(),
                matches: vec![StreamMatch::default()],
                backend_refs: vec![backend_ref("default", "dns", 5353)],
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        backends: vec![BackendCluster {
            name: "dns:5353".to_string(),
            namespace: "default".to_string(),
            protocol: "UDP".to_string(),
            endpoints: vec![BackendEndpoint {
                address: "10.0.0.53".to_string(),
                port: 5353,
                healthy: true,
            }],
            wasm_plugin: None,
                ai_service: None,
                token_policy: None,
        }],
        ..Snapshot::default()
    };

    let selected = snapshot
        .select_stream_backend("default/gw/udp", None)
        .expect("backend");

    assert_eq!(selected.route_kind, RouteKind::Udp);
    assert_eq!(selected.route_name, "dns");
}
