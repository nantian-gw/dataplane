use std::collections::BTreeMap;

use crate::{
    BackendCluster, BackendRef, GrpcMatch, GrpcRoute, GrpcRule, HttpMatch, HttpRoute, HttpRule,
    Listener, ParentRef, RequestMeta, Snapshot, StreamMatch, StreamRoute, StreamRule,
};

use super::{
    RouteSelectionFixture, SnapshotBenchConfig,
    helpers::{bench_backend_port, bench_endpoints},
};

pub fn build_route_selection_fixture(config: SnapshotBenchConfig) -> RouteSelectionFixture {
    let listeners = config.listeners.max(1);
    let routes_per_listener = config.routes_per_listener.max(1);
    let backends_per_route = config.backends_per_route.max(1);
    let endpoints_per_backend = config.endpoints_per_backend.max(1);

    let mut snapshot = Snapshot {
        id: "bench-route-selection".to_string(),
        ..Snapshot::default()
    };

    for listener_index in 0..listeners {
        let http_listener_name = format!("default/gw/http-{listener_index}");
        let grpc_listener_name = format!("default/gw/grpc-{listener_index}");
        let stream_listener_name = format!("default/gw/tls-{listener_index}");
        let http_listener_port = 18_080 + listener_index as u32;
        let grpc_listener_port = 19_090 + listener_index as u32;
        let stream_listener_port = 20_443 + listener_index as u32;
        let http_listener_slot = snapshot.listeners.len();
        snapshot.listeners.push(Listener {
            name: http_listener_name.clone(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: http_listener_port,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            hostnames: vec![format!("http-{listener_index}.example.com")],
            ..Listener::default()
        });
        let grpc_listener_slot = snapshot.listeners.len();
        snapshot.listeners.push(Listener {
            name: grpc_listener_name.clone(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: grpc_listener_port,
            protocol: "LISTENER_PROTOCOL_GRPC".to_string(),
            hostnames: vec![format!("grpc-{listener_index}.example.com")],
            ..Listener::default()
        });
        let stream_listener_slot = snapshot.listeners.len();
        snapshot.listeners.push(Listener {
            name: stream_listener_name.clone(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port: stream_listener_port,
            protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
            ..Listener::default()
        });

        for route_index in 0..routes_per_listener {
            let http_route_name = format!("http-route-{listener_index}-{route_index}");
            let http_host = format!("http-{listener_index}.example.com");
            let http_backend_refs = (0..backends_per_route)
                .map(|backend_index| {
                    let backend_name =
                        format!("http-backend-l{listener_index}-r{route_index}-b{backend_index}");
                    let port = bench_backend_port(
                        10_000,
                        listener_index,
                        route_index,
                        backend_index,
                        backends_per_route,
                        routes_per_listener,
                    );
                    snapshot.backends.push(BackendCluster {
                        name: format!("{backend_name}:{port}"),
                        namespace: "default".to_string(),
                        protocol: "HTTP".to_string(),
                        wasm_plugin: None,
                        ai_service: None,
                        token_policy: None,
                        endpoints: bench_endpoints(
                            listener_index,
                            route_index,
                            backend_index,
                            routes_per_listener,
                            backends_per_route,
                            endpoints_per_backend,
                            port,
                            1,
                        ),

                        circuit_breaker: None,
                    });

                    BackendRef {
                        namespace: "default".to_string(),
                        name: backend_name,
                        port,
                        ..BackendRef::default()
                    }
                })
                .collect::<Vec<_>>();
            snapshot.http_routes.push(HttpRoute {
                name: http_route_name.clone(),
                namespace: "default".to_string(),
                hostnames: vec![http_host.clone()],
                parent_refs: vec![ParentRef {
                    namespace: "default".to_string(),
                    name: format!("gw-http-{listener_index}"),
                    section_name: http_listener_name.clone(),
                    port: http_listener_port,
                    ..ParentRef::default()
                }],
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: format!("/svc/{listener_index}/{route_index}"),
                        path_type: "PathPrefix".to_string(),
                        method: "GET".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs: http_backend_refs,
                    ..HttpRule::default()
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            });
            snapshot.listeners[http_listener_slot]
                .attached_routes
                .push(format!("default/{http_route_name}"));

            let grpc_route_name = format!("grpc-route-{listener_index}-{route_index}");
            let grpc_host = format!("grpc-{listener_index}.example.com");
            let grpc_service = format!("bench.service{listener_index}.Route{route_index}");
            let grpc_backend_refs = (0..backends_per_route)
                .map(|backend_index| {
                    let backend_name =
                        format!("grpc-backend-l{listener_index}-r{route_index}-b{backend_index}");
                    let port = bench_backend_port(
                        20_000,
                        listener_index,
                        route_index,
                        backend_index,
                        backends_per_route,
                        routes_per_listener,
                    );
                    snapshot.backends.push(BackendCluster {
                        name: format!("{backend_name}:{port}"),
                        namespace: "default".to_string(),
                        protocol: "HTTP".to_string(),
                        wasm_plugin: None,
                        ai_service: None,
                        token_policy: None,
                        endpoints: bench_endpoints(
                            listener_index,
                            route_index,
                            backend_index,
                            routes_per_listener,
                            backends_per_route,
                            endpoints_per_backend,
                            port,
                            101,
                        ),

                        circuit_breaker: None,
                    });

                    BackendRef {
                        namespace: "default".to_string(),
                        name: backend_name,
                        port,
                        ..BackendRef::default()
                    }
                })
                .collect::<Vec<_>>();
            snapshot.grpc_routes.push(GrpcRoute {
                name: grpc_route_name.clone(),
                namespace: "default".to_string(),
                hostnames: vec![grpc_host.clone()],
                parent_refs: vec![ParentRef {
                    namespace: "default".to_string(),
                    name: format!("gw-grpc-{listener_index}"),
                    section_name: grpc_listener_name.clone(),
                    port: grpc_listener_port,
                    ..ParentRef::default()
                }],
                rules: vec![GrpcRule {
                    name: String::new(),
                    matches: vec![GrpcMatch {
                        service: grpc_service,
                        method: "Unary".to_string(),
                        match_type: "Exact".to_string(),
                        ..GrpcMatch::default()
                    }],
                    backend_refs: grpc_backend_refs,
                    ..GrpcRule::default()
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            });
            snapshot.listeners[grpc_listener_slot]
                .attached_routes
                .push(format!("default/{grpc_route_name}"));

            let stream_route_name = format!("tls-route-{listener_index}-{route_index}");
            let stream_sni = format!("svc-{listener_index}-{route_index}.example.com");
            let stream_backend_refs = (0..backends_per_route)
                .map(|backend_index| {
                    let backend_name =
                        format!("tls-backend-l{listener_index}-r{route_index}-b{backend_index}");
                    let port = bench_backend_port(
                        30_000,
                        listener_index,
                        route_index,
                        backend_index,
                        backends_per_route,
                        routes_per_listener,
                    );
                    snapshot.backends.push(BackendCluster {
                        name: format!("{backend_name}:{port}"),
                        namespace: "default".to_string(),
                        protocol: "TCP".to_string(),
                        wasm_plugin: None,
                        ai_service: None,
                        token_policy: None,
                        endpoints: bench_endpoints(
                            listener_index,
                            route_index,
                            backend_index,
                            routes_per_listener,
                            backends_per_route,
                            endpoints_per_backend,
                            port,
                            201,
                        ),

                        circuit_breaker: None,
                    });

                    BackendRef {
                        namespace: "default".to_string(),
                        name: backend_name,
                        port,
                        ..BackendRef::default()
                    }
                })
                .collect::<Vec<_>>();
            snapshot.stream_routes.push(StreamRoute {
                name: stream_route_name.clone(),
                namespace: "default".to_string(),
                kind: "ROUTE_KIND_TLS".to_string(),
                parent_refs: vec![ParentRef {
                    namespace: "default".to_string(),
                    name: format!("gw-tls-{listener_index}"),
                    section_name: stream_listener_name.clone(),
                    port: stream_listener_port,
                    ..ParentRef::default()
                }],
                rules: vec![StreamRule {
                    name: String::new(),
                    matches: vec![StreamMatch {
                        port: stream_listener_port,
                        sni_hostname: stream_sni.clone(),
                        ..Default::default()
                    }],
                    backend_refs: stream_backend_refs,
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            });
            snapshot.listeners[stream_listener_slot]
                .attached_routes
                .push(format!("default/{stream_route_name}"));
        }
    }

    snapshot.rebuild_runtime_indexes();

    RouteSelectionFixture {
        snapshot,
        http_request: RequestMeta::with_port(
            Some("http-0.example.com".to_string()),
            18_080,
            "/svc/0/0/items/42?trace=1",
            "GET",
            BTreeMap::new(),
        ),
        grpc_request: RequestMeta::with_port(
            Some("grpc-0.example.com".to_string()),
            19_090,
            "/bench.service0.Route0/Unary",
            "POST",
            BTreeMap::from([(
                "content-type".to_string(),
                vec!["application/grpc+proto".to_string()],
            )]),
        ),
        stream_listener_name: "default/gw/tls-0".to_string(),
        stream_server_name: "svc-0-0.example.com".to_string(),
        expected_http_backend_name: "default/http-backend-l0-r0-b0:10000".to_string(),
        expected_grpc_backend_name: "default/grpc-backend-l0-r0-b0:20000".to_string(),
        expected_stream_backend_name: "default/tls-backend-l0-r0-b0:30000".to_string(),
    }
}
