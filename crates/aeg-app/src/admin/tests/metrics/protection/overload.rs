#[test]
fn render_metrics_exposes_overload_counters() {
    let state = test_state(None);

    let http_listener = HttpAdmissionController::new(
        HttpAdmissionOptions {
            listener_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _http_listener_permit = http_listener
        .try_acquire("web", "Http/default/listener-only")
        .expect("http listener permit");
    assert!(http_listener
        .try_acquire("web", "Http/default/listener-reject")
        .is_err());

    let http_route = HttpAdmissionController::new(
        HttpAdmissionOptions {
            route_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _http_route_permit = http_route
        .try_acquire("web", "Http/default/shared")
        .expect("http route permit");
    assert!(http_route
        .try_acquire("admin", "Http/default/shared")
        .is_err());

    let http_global = HttpAdmissionController::new(
        HttpAdmissionOptions {
            global_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _http_global_permit = http_global
        .try_acquire("edge", "Http/default/global")
        .expect("http global permit");
    assert!(http_global
        .try_acquire("edge-2", "Http/default/global-2")
        .is_err());

    let tcp_listener = TcpAdmissionController::new(
        TcpAdmissionOptions {
            listener_connection_limit: 1,
            ..TcpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _tcp_listener_permit = tcp_listener
        .try_acquire("passthrough")
        .expect("tcp listener permit");
    assert!(tcp_listener.try_acquire("passthrough").is_err());

    let tcp_global = TcpAdmissionController::new(
        TcpAdmissionOptions {
            global_connection_limit: 1,
            ..TcpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _tcp_global_permit = tcp_global
        .try_acquire("tcp-global")
        .expect("tcp global permit");
    assert!(tcp_global.try_acquire("tcp-global-2").is_err());

    let udp_listener = UdpAdmissionController::new(
        UdpAdmissionOptions {
            listener_datagram_limit: 1,
            ..UdpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _udp_listener_permit = udp_listener
        .try_acquire("dns")
        .expect("udp listener permit");
    assert!(udp_listener.try_acquire("dns").is_err());

    let udp_global = UdpAdmissionController::new(
        UdpAdmissionOptions {
            global_datagram_limit: 1,
            ..UdpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _udp_global_permit = udp_global
        .try_acquire("udp-global")
        .expect("udp global permit");
    assert!(udp_global.try_acquire("udp-global-2").is_err());

    let metrics = render_metrics(&state);

    assert!(metrics.contains("nantian_gateway_dataplane_http_global_inflight_current 1"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_http_listener_inflight_current{listener=\"web\"} 1"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_route_inflight_current{route=\"Http/default/shared\"} 1"
    ));
    assert!(metrics
        .contains("nantian_gateway_dataplane_http_overload_rejected_total{scope=\"total\"} 3"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_http_overload_rejected_total{scope=\"global\"} 1"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_http_overload_rejected_total{scope=\"listener\"} 1"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_http_overload_rejected_total{scope=\"route\"} 1"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_overload_rejected_listener_total{listener=\"web\"} 1"
    ));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_http_overload_rejected_route_total{route=\"Http/default/shared\"} 1"
    ));
    assert!(metrics.contains("nantian_gateway_dataplane_tcp_global_connections_current 1"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_tcp_listener_connections_current{listener=\"passthrough\"} 1"
    ));
    assert!(metrics
        .contains("nantian_gateway_dataplane_tcp_overload_rejected_total{scope=\"total\"} 2"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_tcp_overload_rejected_total{scope=\"global\"} 1"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_tcp_overload_rejected_total{scope=\"listener\"} 1"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_tcp_overload_rejected_listener_total{listener=\"passthrough\"} 1"
    ));
    assert!(metrics.contains("nantian_gateway_dataplane_udp_global_datagrams_current 1"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_udp_listener_datagrams_current{listener=\"dns\"} 1"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_udp_overload_rejected_total{scope=\"total\"} 2"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_udp_overload_rejected_total{scope=\"global\"} 1"));
    assert!(metrics
        .contains("nantian_gateway_dataplane_udp_overload_rejected_total{scope=\"listener\"} 1"));
    assert!(metrics.contains(
        "nantian_gateway_dataplane_udp_overload_rejected_listener_total{listener=\"dns\"} 1"
    ));
}
