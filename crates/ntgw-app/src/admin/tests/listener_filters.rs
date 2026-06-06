use super::*;

#[test]
fn listener_filters_and_detail_lookup_work() {
    let snapshot = fixture_snapshot();
    let query = ListenerListQuery {
        protocol: Some("listener_protocol_http".to_string()),
        hostname: Some("app.example.com".to_string()),
        attached_route: Some("default/web".to_string()),
        ..ListenerListQuery::default()
    };

    let listeners = filter_listeners(&snapshot, &query).expect("listener filter");
    assert_eq!(listeners.len(), 1);
    assert_eq!(listeners[0].name, "web");
    assert_eq!(listeners[0].addresses, vec!["192.0.2.10", "gw.example.com"]);

    let detail = find_listener(&snapshot, "passthrough").expect("listener detail");
    assert_eq!(detail.protocol, "LISTENER_PROTOCOL_TLS_PASSTHROUGH");
}
