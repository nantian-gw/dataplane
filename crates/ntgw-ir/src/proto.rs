use std::{collections::BTreeMap, time::Duration};

use ntgw_proto::gateway::control::v1 as proto;
use prost_types::{Duration as ProtoDuration, ListValue, Struct, Value, value::Kind};

use crate::{
    AIServiceAuthConfig, AIServiceConfig, BackendCluster, BackendEndpoint, BackendPolicy,
    BackendRef, BackendSubjectAltName, BackendTlsValidation, ConsistentHashPolicy, CorsFilter,
    DirectResponseFilter, ExtensionFilter, ExternalAuthFilter, ExternalGRPCAuthConfig,
    ExternalHTTPAuthConfig, Filter, Fraction, GrpcMatch, GrpcRoute, GrpcRule, HeaderMatch,
    HeaderModifier, HeaderOperation, HttpMatch, HttpRoute, HttpRule, Listener, LoadBalancingPolicy,
    ParentRef, PathModifier, QueryMatch, RequestMirrorFilter, RequestRedirectFilter, RetryPolicy,
    RouteTimeouts, SecretMaterial, SessionPersistence, Snapshot, StreamMatch, StreamRoute,
    StreamRule, TlsConfig, TokenPolicyConfig, UrlRewriteFilter, WasmPluginConfig,
    WasmSandboxConfig, Workload,
};

mod backend;
mod filters;
mod routes;
mod values;

use self::backend::{backend_from_proto, backend_policy_from_proto, workloads_from_extensions};
use self::routes::{
    backend_tls_from_proto, grpc_rule_from_proto, http_rule_from_proto, parent_ref_from_proto,
    stream_rule_from_proto, tls_from_proto,
};

impl Snapshot {
    pub fn from_proto_without_runtime_indexes(value: proto::ConfigSnapshot) -> Self {
        let workloads = workloads_from_extensions(value.extensions.as_ref());
        let mut backends = Vec::with_capacity(value.backends.len());
        let mut backend_policies = BTreeMap::new();
        for item in value.backends {
            let backend_name = format!("{}/{}", item.namespace, item.name);
            let policy = backend_policy_from_proto(&item);
            if policy.connect_timeout.is_some()
                || policy.request_timeout.is_some()
                || policy.tls_validation.is_some()
                || policy.session_persistence.is_some()
                || policy.load_balancing.is_some()
            {
                backend_policies.insert(backend_name, policy);
            }
            backends.push(backend_from_proto(item));
        }

        Self {
            id: value.id,
            listeners: value
                .listeners
                .into_iter()
                .map(|item| {
                    let protocol = item.protocol().as_str_name().to_string();
                    Listener {
                        name: item.name,
                        address: item.address,
                        addresses: item.addresses,
                        port: item.port,
                        protocol,
                        hostnames: item.hostnames,
                        attached_routes: item.attached_routes,
                        tls: item.tls.map(tls_from_proto),
                        backend_tls: item.backend_tls.map(backend_tls_from_proto),
                        metadata: item.metadata.into_iter().collect(),
                    }
                })
                .collect(),
            http_routes: value
                .http_routes
                .into_iter()
                .map(|item| HttpRoute {
                    name: item.name,
                    namespace: item.namespace,
                    hostnames: item.hostnames,
                    parent_refs: item
                        .parent_refs
                        .into_iter()
                        .map(parent_ref_from_proto)
                        .collect(),
                    rules: item.rules.into_iter().map(http_rule_from_proto).collect(),
                    annotations: item.annotations.into_iter().collect(),
                })
                .collect(),
            grpc_routes: value
                .grpc_routes
                .into_iter()
                .map(|item| GrpcRoute {
                    name: item.name,
                    namespace: item.namespace,
                    hostnames: item.hostnames,
                    parent_refs: item
                        .parent_refs
                        .into_iter()
                        .map(parent_ref_from_proto)
                        .collect(),
                    rules: item.rules.into_iter().map(grpc_rule_from_proto).collect(),
                    annotations: item.annotations.into_iter().collect(),
                })
                .collect(),
            stream_routes: value
                .stream_routes
                .into_iter()
                .map(|item| {
                    let kind = item.kind().as_str_name().to_string();
                    StreamRoute {
                        name: item.name,
                        namespace: item.namespace,
                        kind,
                        parent_refs: item
                            .parent_refs
                            .into_iter()
                            .map(parent_ref_from_proto)
                            .collect(),
                        rules: item.rules.into_iter().map(stream_rule_from_proto).collect(),
                        annotations: item.annotations.into_iter().collect(),
                    }
                })
                .collect(),
            backends,
            backend_policies,
            secrets: value
                .secrets
                .into_iter()
                .map(|item| SecretMaterial {
                    namespace: item.namespace,
                    name: item.name,
                    cert_pem: item.cert_pem,
                    key_pem: item.key_pem,
                })
                .collect(),
            workloads,
            selection_state: Default::default(),
            ..Self::default()
        }
    }
}

impl From<proto::ConfigSnapshot> for Snapshot {
    fn from(value: proto::ConfigSnapshot) -> Self {
        let mut snapshot = Self::from_proto_without_runtime_indexes(value);
        snapshot.rebuild_runtime_indexes();
        snapshot
    }
}
