fn direct_response_snapshot(listener_port: u16, path: &str) -> ntgw_ir::SharedSnapshot {
    let shared = Snapshot::shared();
    shared.store(Arc::new(Snapshot {
        listeners: vec![Listener {
            name: "default/gw/http".to_string().into(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: listener_port as u32,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
            attached_routes: vec!["default/route".to_string()],
            ..Listener::default()
        }],
        http_routes: vec![HttpRoute {
            name: "route".to_string().into(),
            namespace: "default".to_string().into(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                namespace: "default".to_string().into(),
                name: "gw".to_string().into(),
                section_name: String::new(),
                port: listener_port as u32,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: path.to_string(),
                    path_type: "Exact".to_string(),
                    ..HttpMatch::default()
                }],
                filters: vec![
                    Filter {
                        filter_type: "ExtensionRef".to_string(),
                        extension_ref: Some(ExtensionFilter {
                            resolved: true,
                            extension_type: "DirectResponse".to_string(),
                            direct_response: Some(DirectResponseFilter {
                                status_code: 202,
                                body: "direct response".to_string(),
                                content_type: "text/plain".to_string(),
                                headers: vec![HeaderOperation {
                                    name: "x-direct-source".to_string().into(),
                                    value: "extension".to_string(),
                                }],
                            }),
                            ..ExtensionFilter::default()
                        }),
                        ..Filter::default()
                    },
                    Filter {
                        filter_type: "ResponseHeaderModifier".to_string(),
                        header_modifier: Some(HeaderModifier {
                            add: vec![HeaderOperation {
                                name: "x-lifecycle-stage".to_string().into(),
                                value: "request-filter".to_string(),
                            }],
                            ..HeaderModifier::default()
                        }),
                        ..Filter::default()
                    },
                ],
                backend_refs: Vec::new(),
                ..HttpRule::default()
            }],
            labels: BTreeMap::new(),
            annotations: BTreeMap::new(),
        }],
        ..Snapshot::default()
    }));
    let mut s = (**shared.load()).clone();
    s.rebuild_runtime_indexes();
    shared.store(Arc::new(s));
    shared
}
