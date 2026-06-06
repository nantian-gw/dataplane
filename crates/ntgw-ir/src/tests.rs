use std::{
    collections::BTreeMap,
    sync::{Arc, Barrier},
    time::{Duration, Instant},
};

use ntgw_proto::gateway::control::v1 as proto;
use prost_types::{value::Kind, ListValue, Struct, Value};

use crate::{
    BackendCluster, BackendEndpoint, BackendRef, Filter, GrpcMatch, GrpcRoute, GrpcRule,
    HeaderMatch, HttpMatch, HttpRoute, HttpRule, Listener, QueryMatch, RequestMeta, RouteKind,
    RuntimeResourceRef, SecretMaterial, SessionPersistence, Snapshot, SnapshotSignal, StreamMatch,
    StreamRoute, StreamRule, TlsRouteMode, Workload, PASSIVE_EJECTION_CONSECUTIVE_FAILURES,
    PASSIVE_EJECTION_COOLDOWN,
};

include!("tests_selection.rs");

include!("tests_proto.rs");

include!("tests_weighted.rs");

include!("tests_stream.rs");

include!("tests_load_balancing.rs");

include!("tests_bench.rs");

include!("tests_property.rs");

fn backend_ref(namespace: &str, name: &str, port: u32) -> BackendRef {
    weighted_backend_ref(namespace, name, port, 1)
}

fn weighted_backend_ref(namespace: &str, name: &str, port: u32, weight: u32) -> BackendRef {
    BackendRef {
        namespace: namespace.to_string(),
        name: name.to_string(),
        port,
        weight,
        ..BackendRef::default()
    }
}

fn collect_http_backends(
    snapshot: &Snapshot,
    request: &RequestMeta,
    attempts: usize,
) -> Vec<String> {
    (0..attempts)
        .map(|_| {
            snapshot
                .select_backend(request)
                .expect("backend")
                .backend_name
        })
        .collect()
}

fn collect_http_addresses(
    snapshot: &Snapshot,
    request: &RequestMeta,
    attempts: usize,
) -> Vec<String> {
    (0..attempts)
        .map(|_| {
            snapshot
                .select_backend(request)
                .expect("backend")
                .backend
                .address
        })
        .collect()
}

fn collect_stream_backends(
    snapshot: &Snapshot,
    listener_name: &str,
    server_name: Option<&str>,
    attempts: usize,
) -> Vec<String> {
    (0..attempts)
        .map(|_| {
            snapshot
                .select_stream_backend(listener_name, server_name)
                .expect("backend")
                .backend_name
        })
        .collect()
}

fn string_proto_value(value: &str) -> Value {
    Value {
        kind: Some(Kind::StringValue(value.to_string())),
    }
}

fn list_proto_value(values: Vec<Value>) -> Value {
    Value {
        kind: Some(Kind::ListValue(ListValue { values })),
    }
}

fn struct_proto_value(fields: BTreeMap<String, Value>) -> Value {
    Value {
        kind: Some(Kind::StructValue(Struct { fields })),
    }
}

fn number_proto_value(value: f64) -> Value {
    Value {
        kind: Some(Kind::NumberValue(value)),
    }
}

fn bool_proto_value(value: bool) -> Value {
    Value {
        kind: Some(Kind::BoolValue(value)),
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
