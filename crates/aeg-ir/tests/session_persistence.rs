use std::collections::BTreeMap;

use aeg_ir::{
    BackendCluster, BackendEndpoint, BackendPolicy, BackendRef, GrpcMatch, GrpcRoute, GrpcRule,
    HttpRoute, HttpRule, PersistentSessionTarget, RequestMeta, SessionPersistence, Snapshot,
};
use aeg_proto::gateway::control::v1 as proto;

#[path = "session_persistence/grpc_routes.rs"]
mod grpc_routes;
#[path = "session_persistence/http_routes.rs"]
mod http_routes;
#[path = "session_persistence/proto_decode.rs"]
mod proto_decode;

fn weighted_backend_ref(namespace: &str, name: &str, port: u32, weight: u32) -> BackendRef {
    BackendRef {
        namespace: namespace.to_string(),
        name: name.to_string(),
        port,
        weight,
        ..BackendRef::default()
    }
}

fn session_policy(name: &str) -> SessionPersistence {
    SessionPersistence {
        session_name: name.to_string(),
        session_type: "Cookie".to_string(),
        ..SessionPersistence::default()
    }
}

fn session_target(backend_name: &str, address: &str, port: u32) -> PersistentSessionTarget {
    PersistentSessionTarget {
        backend_name: backend_name.to_string(),
        endpoint: BackendEndpoint {
            address: address.to_string(),
            port,
            healthy: true,
        },
    }
}

fn headers(values: &[(&str, &str)]) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, value) in values {
        out.entry((*name).to_string())
            .or_default()
            .push((*value).to_string());
    }
    out
}
