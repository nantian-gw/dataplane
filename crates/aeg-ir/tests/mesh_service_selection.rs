use std::collections::BTreeMap;

use aeg_ir::{
    BackendCluster, BackendEndpoint, BackendRef, Filter, GrpcRoute, GrpcRule, HeaderModifier,
    HttpMatch, HttpRoute, HttpRule, Listener, ParentRef, RequestMeta, Snapshot, Workload,
};

#[path = "mesh_service_selection/cross_namespace.rs"]
mod cross_namespace;
#[path = "mesh_service_selection/excluded_ports.rs"]
mod excluded_ports;
#[path = "mesh_service_selection/no_fallback.rs"]
mod no_fallback;
#[path = "mesh_service_selection/weighted_grpc.rs"]
mod weighted_grpc;

fn mesh_listener(
    namespace: &str,
    name: &str,
    service_port: u32,
    listen_port: u32,
    protocol: &str,
    attached_routes: &[&str],
) -> Listener {
    let mut listener = Listener {
        name: format!("mesh/{namespace}/{name}/{listen_port}"),
        address: "0.0.0.0".to_string(),
        port: listen_port,
        protocol: protocol.to_string(),
        hostnames: vec![],
        attached_routes: attached_routes
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
        tls: None,
        ..Listener::default()
    };
    listener
        .metadata
        .insert("nantian.dev/frontend-kind".to_string(), "Service".to_string());
    listener.metadata.insert(
        "nantian.dev/frontend-namespace".to_string(),
        namespace.to_string(),
    );
    listener
        .metadata
        .insert("nantian.dev/frontend-name".to_string(), name.to_string());
    listener
        .metadata
        .insert("nantian.dev/frontend-port".to_string(), service_port.to_string());
    listener
}
