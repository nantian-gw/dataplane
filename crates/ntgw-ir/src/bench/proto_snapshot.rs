use ntgw_proto::gateway::control::v1 as proto;

use super::{
    ProtoSnapshotFixture, SnapshotBenchConfig,
    helpers::{bench_backend_port, bench_proto_endpoints},
};

pub fn build_proto_snapshot_fixture(config: SnapshotBenchConfig) -> ProtoSnapshotFixture {
    let listeners = config.listeners.max(1);
    let routes_per_listener = config.routes_per_listener.max(1);
    let backends_per_route = config.backends_per_route.max(1);
    let endpoints_per_backend = config.endpoints_per_backend.max(1);

    let mut snapshot = proto::ConfigSnapshot {
        id: "bench-proto-snapshot".to_string(),
        ..proto::ConfigSnapshot::default()
    };

    for listener_index in 0..listeners {
        let http_listener_name = format!("default/gw/http-{listener_index}");
        let grpc_listener_name = format!("default/gw/grpc-{listener_index}");
        let stream_listener_name = format!("default/gw/tls-{listener_index}");
        let http_listener_port = 18_080 + listener_index as u32;
        let grpc_listener_port = 19_090 + listener_index as u32;
        let stream_listener_port = 20_443 + listener_index as u32;

        let http_listener_slot = snapshot.listeners.len();
        snapshot.listeners.push(proto::Listener {
            name: http_listener_name.clone(),
            address: "127.0.0.1".to_string(),
            port: http_listener_port,
            protocol: proto::ListenerProtocol::ListenerHttp as i32,
            hostnames: vec![format!("http-{listener_index}.example.com")],
            attached_routes: Vec::new(),
            addresses: vec!["127.0.0.1".to_string()],
            ..proto::Listener::default()
        });
        let grpc_listener_slot = snapshot.listeners.len();
        snapshot.listeners.push(proto::Listener {
            name: grpc_listener_name.clone(),
            address: "127.0.0.1".to_string(),
            port: grpc_listener_port,
            protocol: proto::ListenerProtocol::ListenerGrpc as i32,
            hostnames: vec![format!("grpc-{listener_index}.example.com")],
            attached_routes: Vec::new(),
            addresses: vec!["127.0.0.1".to_string()],
            ..proto::Listener::default()
        });
        let stream_listener_slot = snapshot.listeners.len();
        snapshot.listeners.push(proto::Listener {
            name: stream_listener_name.clone(),
            address: "127.0.0.1".to_string(),
            port: stream_listener_port,
            protocol: proto::ListenerProtocol::ListenerTlsPassthrough as i32,
            attached_routes: Vec::new(),
            addresses: vec!["127.0.0.1".to_string()],
            ..proto::Listener::default()
        });

        for route_index in 0..routes_per_listener {
            let http_route_name = format!("http-route-{listener_index}-{route_index}");
            let grpc_route_name = format!("grpc-route-{listener_index}-{route_index}");
            let stream_route_name = format!("tls-route-{listener_index}-{route_index}");
            snapshot.listeners[http_listener_slot]
                .attached_routes
                .push(format!("default/{http_route_name}"));
            snapshot.listeners[grpc_listener_slot]
                .attached_routes
                .push(format!("default/{grpc_route_name}"));
            snapshot.listeners[stream_listener_slot]
                .attached_routes
                .push(format!("default/{stream_route_name}"));

            snapshot.http_routes.push(proto::HttpRoute {
                name: http_route_name,
                namespace: "default".to_string(),
                hostnames: vec![format!("http-{listener_index}.example.com")],
                labels: std::collections::HashMap::from([(
                    "team".to_string(),
                    format!("http-team-{listener_index}-{route_index}"),
                )]),
                parent_refs: vec![proto::ParentRef {
                    namespace: "default".to_string(),
                    name: format!("gw-http-{listener_index}"),
                    section_name: http_listener_name.clone(),
                    port: http_listener_port,
                    ..proto::ParentRef::default()
                }],
                rules: vec![proto::HttpRule {
                    name: String::new(),
                    matches: vec![proto::HttpMatch {
                        path: format!("/svc/{listener_index}/{route_index}"),
                        path_type: "PathPrefix".to_string(),
                        method: "GET".to_string(),
                        ..proto::HttpMatch::default()
                    }],
                    backend_refs: (0..backends_per_route)
                        .map(|backend_index| {
                            let backend_name = format!(
                                "http-backend-l{listener_index}-r{route_index}-b{backend_index}"
                            );
                            let port = bench_backend_port(
                                10_000,
                                listener_index,
                                route_index,
                                backend_index,
                                backends_per_route,
                                routes_per_listener,
                            );
                            snapshot.backends.push(proto::BackendCluster {
                                ai_service: None,
                                token_policy: None,
                                name: format!("{backend_name}:{port}"),
                                namespace: "default".to_string(),
                                protocol: "HTTP".to_string(),
                                endpoints: bench_proto_endpoints(
                                    listener_index,
                                    route_index,
                                    backend_index,
                                    routes_per_listener,
                                    backends_per_route,
                                    endpoints_per_backend,
                                    port,
                                    1,
                                ),
                                wasm_plugin: None,
                                ..proto::BackendCluster::default()
                            });

                            proto::BackendRef {
                                namespace: "default".to_string(),
                                name: backend_name,
                                port,
                                ..proto::BackendRef::default()
                            }
                        })
                        .collect(),
                    ..proto::HttpRule::default()
                }],
                ..proto::HttpRoute::default()
            });

            snapshot.grpc_routes.push(proto::GrpcRoute {
                name: grpc_route_name,
                namespace: "default".to_string(),
                hostnames: vec![format!("grpc-{listener_index}.example.com")],
                labels: std::collections::HashMap::from([(
                    "team".to_string(),
                    format!("grpc-team-{listener_index}-{route_index}"),
                )]),
                parent_refs: vec![proto::ParentRef {
                    namespace: "default".to_string(),
                    name: format!("gw-grpc-{listener_index}"),
                    section_name: grpc_listener_name.clone(),
                    port: grpc_listener_port,
                    ..proto::ParentRef::default()
                }],
                rules: vec![proto::GrpcRule {
                    name: String::new(),
                    matches: vec![proto::GrpcMatch {
                        service: format!("bench.service{listener_index}.Route{route_index}"),
                        method: "Unary".to_string(),
                        match_type: "Exact".to_string(),
                        ..proto::GrpcMatch::default()
                    }],
                    backend_refs: (0..backends_per_route)
                        .map(|backend_index| {
                            let backend_name = format!(
                                "grpc-backend-l{listener_index}-r{route_index}-b{backend_index}"
                            );
                            let port = bench_backend_port(
                                20_000,
                                listener_index,
                                route_index,
                                backend_index,
                                backends_per_route,
                                routes_per_listener,
                            );
                            snapshot.backends.push(proto::BackendCluster {
                                ai_service: None,
                                token_policy: None,
                                name: format!("{backend_name}:{port}"),
                                namespace: "default".to_string(),
                                protocol: "HTTP".to_string(),
                                endpoints: bench_proto_endpoints(
                                    listener_index,
                                    route_index,
                                    backend_index,
                                    routes_per_listener,
                                    backends_per_route,
                                    endpoints_per_backend,
                                    port,
                                    101,
                                ),
                                wasm_plugin: None,
                                ..proto::BackendCluster::default()
                            });

                            proto::BackendRef {
                                namespace: "default".to_string(),
                                name: backend_name,
                                port,
                                ..proto::BackendRef::default()
                            }
                        })
                        .collect(),
                    ..proto::GrpcRule::default()
                }],
                ..proto::GrpcRoute::default()
            });

            snapshot.stream_routes.push(proto::StreamRoute {
                name: stream_route_name,
                namespace: "default".to_string(),
                kind: proto::RouteKind::Tls as i32,
                labels: std::collections::HashMap::from([(
                    "team".to_string(),
                    format!("stream-team-{listener_index}-{route_index}"),
                )]),
                parent_refs: vec![proto::ParentRef {
                    namespace: "default".to_string(),
                    name: format!("gw-tls-{listener_index}"),
                    section_name: stream_listener_name.clone(),
                    port: stream_listener_port,
                    ..proto::ParentRef::default()
                }],
                rules: vec![proto::StreamRule {
                    name: String::new(),
                    matches: vec![proto::StreamMatch {
                        port: stream_listener_port,
                        sni_hostname: format!("svc-{listener_index}-{route_index}.example.com"),
                        mode: 0,
                    }],
                    backend_refs: (0..backends_per_route)
                        .map(|backend_index| {
                            let backend_name = format!(
                                "tls-backend-l{listener_index}-r{route_index}-b{backend_index}"
                            );
                            let port = bench_backend_port(
                                30_000,
                                listener_index,
                                route_index,
                                backend_index,
                                backends_per_route,
                                routes_per_listener,
                            );
                            snapshot.backends.push(proto::BackendCluster {
                                ai_service: None,
                                token_policy: None,
                                name: format!("{backend_name}:{port}"),
                                namespace: "default".to_string(),
                                protocol: "TCP".to_string(),
                                endpoints: bench_proto_endpoints(
                                    listener_index,
                                    route_index,
                                    backend_index,
                                    routes_per_listener,
                                    backends_per_route,
                                    endpoints_per_backend,
                                    port,
                                    201,
                                ),
                                wasm_plugin: None,
                                ..proto::BackendCluster::default()
                            });

                            proto::BackendRef {
                                namespace: "default".to_string(),
                                name: backend_name,
                                port,
                                ..proto::BackendRef::default()
                            }
                        })
                        .collect(),
                }],
                ..proto::StreamRoute::default()
            });
        }
    }

    ProtoSnapshotFixture {
        expected_listener_count: snapshot.listeners.len(),
        expected_http_routes: snapshot.http_routes.len(),
        expected_grpc_routes: snapshot.grpc_routes.len(),
        expected_stream_routes: snapshot.stream_routes.len(),
        expected_backends: snapshot.backends.len(),
        snapshot,
    }
}
