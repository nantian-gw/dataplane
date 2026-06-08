use super::*;

#[tokio::test]
async fn overload_view_returns_named_current_and_rejection_counts() {
    let state = test_state(Some("top-secret"));

    let http_listener = HttpAdmissionController::new(
        HttpAdmissionOptions {
            listener_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _http_listener_permit = http_listener
        .try_acquire("web", "Http/default/listener-current")
        .expect("http listener permit");
    assert!(
        http_listener
            .try_acquire("web", "Http/default/listener-rejected")
            .is_err()
    );

    let http_route = HttpAdmissionController::new(
        HttpAdmissionOptions {
            route_inflight_limit: 1,
            ..HttpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _http_route_permit = http_route
        .try_acquire("admin", "Http/default/shared")
        .expect("http route permit");
    assert!(
        http_route
            .try_acquire("edge", "Http/default/shared")
            .is_err()
    );

    let tcp = TcpAdmissionController::new(
        TcpAdmissionOptions {
            listener_connection_limit: 1,
            ..TcpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _tcp_permit = tcp.try_acquire("passthrough").expect("tcp permit");
    assert!(tcp.try_acquire("passthrough").is_err());

    let udp = UdpAdmissionController::new(
        UdpAdmissionOptions {
            listener_datagram_limit: 1,
            ..UdpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _udp_permit = udp.try_acquire("dns").expect("udp permit");
    assert!(udp.try_acquire("dns").is_err());

    let app = super::build_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/overload")
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let payload: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(payload["httpListenerInflightCurrent"]["web"], 1);
    assert_eq!(
        payload["httpRouteInflightCurrent"]["Http/default/shared"],
        1
    );
    assert_eq!(payload["httpRejectedListenerByName"]["web"], 1);
    assert_eq!(payload["httpRejectedRouteByName"]["Http/default/shared"], 1);
    assert_eq!(payload["tcpListenerConnectionsCurrent"]["passthrough"], 1);
    assert_eq!(payload["tcpRejectedListenerByName"]["passthrough"], 1);
    assert_eq!(payload["udpListenerDatagramsCurrent"]["dns"], 1);
    assert_eq!(payload["udpRejectedListenerByName"]["dns"], 1);
}
