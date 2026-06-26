use std::collections::BTreeMap;

use crate::{
    BackendCluster, BackendEndpoint, BackendRef, HttpMatch, HttpRoute, HttpRule, Listener,
    ParentRef, RequestMeta, SecretMaterial, Snapshot, Workload,
};

use super::{
    SnapshotBenchConfig, SnapshotSwitchFixture,
    helpers::{bench_backend_port, bench_ipv4},
};

pub fn build_snapshot_switch_fixture(config: SnapshotBenchConfig) -> SnapshotSwitchFixture {
    let listeners = config.listeners.max(1);
    let routes_per_listener = config.routes_per_listener.max(1);
    let backends_per_route = config.backends_per_route.max(1);
    let endpoints_per_backend = config.endpoints_per_backend.max(1);

    let mut current = Snapshot {
        id: "bench-current".to_string(),
        ..Snapshot::default()
    };
    let mut next = Snapshot {
        id: "bench-next".to_string(),
        ..Snapshot::default()
    };

    for listener_index in 0..listeners {
        let host = format!("listener-{listener_index}.example.com");
        let port = 18_080 + listener_index as u32;
        let listener_name = format!("default/gw/listener-{listener_index}");
        let listener = Listener {
            name: listener_name.clone(),
            address: "127.0.0.1".to_string(),
            addresses: vec!["127.0.0.1".to_string()],
            port,
            protocol: "LISTENER_PROTOCOL_HTTP".to_string(),
            hostnames: vec![host.clone()],
            ..Listener::default()
        };
        current.listeners.push(listener.clone());
        next.listeners.push(listener);

        let secret = SecretMaterial {
            namespace: "default".to_string(),
            name: format!("bench-cert-{listener_index}"),
            cert_pem: format!("CERT-{listener_index}"),
            key_pem: format!("KEY-{listener_index}"),
        };
        current.secrets.push(secret.clone());
        next.secrets.push(secret);

        let workload = Workload {
            namespace: "bench".to_string(),
            name: format!("workload-{listener_index}"),
            ip: bench_ipv4(listener_index, 1),
        };
        current.workloads.push(workload.clone());
        next.workloads.push(workload);

        for route_index in 0..routes_per_listener {
            let route_name = format!("route-{listener_index}-{route_index}");
            let backend_refs = (0..backends_per_route)
                .map(|backend_index| {
                    let backend_name =
                        format!("backend-l{listener_index}-r{route_index}-b{backend_index}");
                    let port = bench_backend_port(
                        10_000,
                        listener_index,
                        route_index,
                        backend_index,
                        backends_per_route,
                        routes_per_listener,
                    );
                    let current_cluster = BackendCluster {
                        name: format!("{backend_name}:{port}"),
                        namespace: "default".to_string(),
                        protocol: "HTTP".to_string(),
                        wasm_plugin: None,
                        ai_service: None,
                        token_policy: None,
                        endpoints: (0..endpoints_per_backend)
                            .map(|endpoint_index| BackendEndpoint {
                                address: bench_ipv4(
                                    listener_index * routes_per_listener * backends_per_route
                                        + route_index * backends_per_route
                                        + backend_index,
                                    endpoint_index + 10,
                                ),
                                port,
                                healthy: true,
                            })
                            .collect(),

                        circuit_breaker: None,
                    };
                    let mut next_cluster = current_cluster.clone();
                    for (endpoint_index, endpoint) in next_cluster.endpoints.iter_mut().enumerate()
                    {
                        endpoint.address = bench_ipv4(
                            listeners * routes_per_listener * backends_per_route
                                + listener_index * routes_per_listener * backends_per_route
                                + route_index * backends_per_route
                                + backend_index,
                            endpoint_index + 10,
                        );
                    }
                    current.backends.push(current_cluster);
                    next.backends.push(next_cluster);

                    BackendRef {
                        namespace: "default".to_string(),
                        name: backend_name,
                        port,
                        ..BackendRef::default()
                    }
                })
                .collect::<Vec<_>>();

            let route = HttpRoute {
                name: route_name.clone(),
                namespace: "default".to_string(),
                hostnames: vec![host.clone()],
                parent_refs: vec![ParentRef {
                    namespace: "default".to_string(),
                    name: format!("gw-{listener_index}"),
                    section_name: listener_name.clone(),
                    port,
                    ..ParentRef::default()
                }],
                rules: vec![HttpRule {
                    name: String::new(),
                    matches: vec![HttpMatch {
                        path: format!("^/svc/{listener_index}/{route_index}/items/[0-9]+$"),
                        path_type: "RegularExpression".to_string(),
                        method: "GET".to_string(),
                        ..HttpMatch::default()
                    }],
                    backend_refs,
                    ..HttpRule::default()
                }],
                labels: BTreeMap::new(),
                annotations: BTreeMap::new(),
            };
            current.http_routes.push(route.clone());
            next.http_routes.push(route);
            current.listeners[listener_index]
                .attached_routes
                .push(format!("default/{route_name}"));
            next.listeners[listener_index]
                .attached_routes
                .push(format!("default/{route_name}"));
        }
    }

    current.rebuild_runtime_indexes();

    SnapshotSwitchFixture {
        current,
        next,
        probe_request: RequestMeta::new(
            Some("listener-0.example.com".to_string()),
            "/svc/0/0/items/42",
            "GET",
            BTreeMap::new(),
        ),
        expected_backend_name: "default/backend-l0-r0-b0:10000".to_string(),
    }
}
