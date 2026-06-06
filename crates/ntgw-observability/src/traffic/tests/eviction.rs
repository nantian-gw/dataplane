#[test]
fn traffic_state_eviction_prefers_stale_entries() {
    let mut state = TrafficState::default();

    state.observe_node("listener:a".to_string(), None, 10, 20, 1, 2);
    state.observe_node("listener:b".to_string(), None, 10, 20, 2, 2);
    state.observe_node("listener:a".to_string(), None, 10, 20, 3, 2);
    state.observe_node("listener:c".to_string(), None, 10, 20, 4, 2);

    assert!(state.nodes.contains_key("listener:a"));
    assert!(state.nodes.contains_key("listener:c"));
    assert!(!state.nodes.contains_key("listener:b"));

    state.observe_edge(
        ObservedEdge {
            edge_id: "edge:a".to_string(),
            source: "listener:a".to_string(),
            target: "route:a".to_string(),
            bytes_received: 10,
            bytes_sent: 20,
        },
        1,
        2,
    );
    state.observe_edge(
        ObservedEdge {
            edge_id: "edge:b".to_string(),
            source: "listener:b".to_string(),
            target: "route:b".to_string(),
            bytes_received: 10,
            bytes_sent: 20,
        },
        2,
        2,
    );
    state.observe_edge(
        ObservedEdge {
            edge_id: "edge:a".to_string(),
            source: "listener:a".to_string(),
            target: "route:a".to_string(),
            bytes_received: 10,
            bytes_sent: 20,
        },
        3,
        2,
    );
    state.observe_edge(
        ObservedEdge {
            edge_id: "edge:c".to_string(),
            source: "listener:c".to_string(),
            target: "route:c".to_string(),
            bytes_received: 10,
            bytes_sent: 20,
        },
        4,
        2,
    );

    assert!(state.edges.contains_key("edge:a"));
    assert!(state.edges.contains_key("edge:c"));
    assert!(!state.edges.contains_key("edge:b"));
}
