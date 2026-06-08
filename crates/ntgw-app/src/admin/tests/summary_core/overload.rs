use super::*;

#[test]
fn summary_view_exposes_overload_summary() {
    let state = test_state(None);

    let http = HttpAdmissionController::new(
        HttpAdmissionOptions {
            global_inflight_limit: 1,
            listener_inflight_limit: 1,
            route_inflight_limit: 1,
        },
        state.overload.clone(),
    );
    let _http_permit = http
        .try_acquire("default/gw/http", "Http/default/route")
        .expect("http permit");
    assert!(
        http.try_acquire("default/gw/http", "Http/default/route-2")
            .is_err()
    );

    let tcp = TcpAdmissionController::new(
        TcpAdmissionOptions {
            global_connection_limit: 1,
            ..TcpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _tcp_permit = tcp.try_acquire("default/gw/tcp").expect("tcp permit");
    assert!(tcp.try_acquire("default/gw/tcp-2").is_err());

    let udp = UdpAdmissionController::new(
        UdpAdmissionOptions {
            listener_datagram_limit: 1,
            ..UdpAdmissionOptions::default()
        },
        state.overload.clone(),
    );
    let _udp_permit = udp.try_acquire("default/gw/udp").expect("udp permit");
    assert!(udp.try_acquire("default/gw/udp").is_err());

    let value = build_summary_value(&state);

    assert_eq!(value["overloadHttpRejectedTotal"], 1);
    assert_eq!(value["overloadTcpRejectedTotal"], 1);
    assert_eq!(value["overloadUdpRejectedTotal"], 1);
    assert_eq!(
        value["overloadOverview"],
        serde_json::json!({
            "schemaVersion": 1,
            "http": {
                "current": {
                    "globalInflight": 1,
                    "listenerInflight": 1,
                    "routeInflight": 1,
                    "listenerInflightByName": {
                        "default/gw/http": 1,
                    },
                    "routeInflightByName": {
                        "Http/default/route": 1,
                    },
                },
                "rejected": {
                    "total": 1,
                    "global": 1,
                    "listener": 0,
                    "route": 0,
                    "listenerByName": {},
                    "routeByName": {},
                },
            },
            "tcp": {
                "current": {
                    "globalConnections": 1,
                    "listenerConnections": 0,
                    "listenerConnectionsByName": {},
                },
                "rejected": {
                    "total": 1,
                    "global": 1,
                    "listener": 0,
                    "listenerByName": {},
                },
            },
            "udp": {
                "current": {
                    "globalDatagrams": 0,
                    "listenerDatagrams": 1,
                    "listenerDatagramsByName": {
                        "default/gw/udp": 1,
                    },
                },
                "rejected": {
                    "total": 1,
                    "global": 0,
                    "listener": 1,
                    "listenerByName": {
                        "default/gw/udp": 1,
                    },
                },
            },
        })
    );
}
