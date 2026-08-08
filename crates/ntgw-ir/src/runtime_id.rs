use std::{
    collections::{HashMap, hash_map::Entry},
    fmt,
};

use crate::{BackendEndpoint, EndpointRuntimeKey, Snapshot};

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeId(u64);

impl RuntimeId {
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn parse_hex(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.len() != 16 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let parsed = u64::from_str_radix(value, 16).ok()?;
        (parsed != 0).then_some(Self(parsed))
    }
}

impl fmt::Display for RuntimeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeIdIndex {
    listeners: HashMap<String, RuntimeId>,
    http_routes: HashMap<String, RuntimeId>,
    grpc_routes: HashMap<String, RuntimeId>,
    stream_routes: HashMap<String, RuntimeId>,
    http_rules: HashMap<String, RuntimeId>,
    grpc_rules: HashMap<String, RuntimeId>,
    stream_rules: HashMap<String, RuntimeId>,
    backends: HashMap<String, RuntimeId>,
    endpoints: HashMap<EndpointRuntimeKey, RuntimeId>,
    resource_refs: HashMap<RuntimeId, RuntimeResourceRef>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RuntimeResourceRef {
    Listener {
        name: String,
    },
    HttpRoute {
        namespace: String,
        name: String,
    },
    GrpcRoute {
        namespace: String,
        name: String,
    },
    StreamRoute {
        kind: String,
        namespace: String,
        name: String,
    },
    HttpRule {
        namespace: String,
        name: String,
        rule_index: usize,
    },
    GrpcRule {
        namespace: String,
        name: String,
        rule_index: usize,
    },
    StreamRule {
        kind: String,
        namespace: String,
        name: String,
        rule_index: usize,
    },
    Backend {
        name: String,
    },
    Endpoint {
        backend_name: String,
        address: String,
        port: u32,
    },
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct SelectedBackendRuntimeIds {
    pub listener: Option<RuntimeId>,
    pub route: Option<RuntimeId>,
    pub rule: Option<RuntimeId>,
    pub backend: Option<RuntimeId>,
    pub endpoint: Option<RuntimeId>,
}

impl RuntimeIdIndex {
    #[must_use]
    pub fn listener(&self, listener_name: &str) -> Option<RuntimeId> {
        self.listeners.get(listener_name).copied()
    }

    #[must_use]
    pub fn http_route(&self, namespace: &str, name: &str) -> Option<RuntimeId> {
        self.http_routes
            .get(&namespaced_key(namespace, name))
            .copied()
    }

    #[must_use]
    pub fn grpc_route(&self, namespace: &str, name: &str) -> Option<RuntimeId> {
        self.grpc_routes
            .get(&namespaced_key(namespace, name))
            .copied()
    }

    #[must_use]
    pub fn stream_route(&self, kind: &str, namespace: &str, name: &str) -> Option<RuntimeId> {
        self.stream_routes
            .get(&stream_route_key(kind, namespace, name))
            .copied()
    }

    #[must_use]
    pub fn http_rule(&self, namespace: &str, name: &str, rule_index: usize) -> Option<RuntimeId> {
        self.http_rules
            .get(&rule_key(&namespaced_key(namespace, name), rule_index))
            .copied()
    }

    #[must_use]
    pub fn grpc_rule(&self, namespace: &str, name: &str, rule_index: usize) -> Option<RuntimeId> {
        self.grpc_rules
            .get(&rule_key(&namespaced_key(namespace, name), rule_index))
            .copied()
    }

    pub fn stream_rule(
        &self,
        kind: &str,
        namespace: &str,
        name: &str,
        rule_index: usize,
    ) -> Option<RuntimeId> {
        self.stream_rules
            .get(&rule_key(
                &stream_route_key(kind, namespace, name),
                rule_index,
            ))
            .copied()
    }

    #[must_use]
    pub fn backend(&self, backend_name: &str) -> Option<RuntimeId> {
        self.backends.get(backend_name).copied()
    }

    #[must_use]
    pub fn endpoint(&self, backend_name: &str, endpoint: &BackendEndpoint) -> Option<RuntimeId> {
        self.endpoints
            .get(&EndpointRuntimeKey::new(backend_name, endpoint))
            .copied()
    }

    #[must_use]
    pub fn resource_ref(&self, runtime_id: RuntimeId) -> Option<RuntimeResourceRef> {
        self.resource_refs.get(&runtime_id).cloned()
    }

    pub fn selected_backend(
        &self,
        snapshot: &Snapshot,
        selected: &crate::SelectedBackend,
    ) -> SelectedBackendRuntimeIds {
        SelectedBackendRuntimeIds {
            listener: self.listener(selected.listener_name.as_str()),
            route: selected.route_kind_runtime_id(snapshot),
            rule: selected.rule_kind_runtime_id(snapshot),
            backend: self.backend(selected.backend_name.as_str()),
            endpoint: self.endpoint(selected.backend_name.as_str(), &selected.backend),
        }
    }

    pub(crate) fn from_snapshot(snapshot: &Snapshot) -> Self {
        let mut index = Self::default();

        for listener in &snapshot.listeners {
            let runtime_id = stable_runtime_id(&["listener", listener.name.as_str()]);
            index.listeners.insert(listener.name.clone(), runtime_id);
            index.insert_resource_ref(
                runtime_id,
                RuntimeResourceRef::Listener {
                    name: listener.name.clone(),
                },
            );
        }

        for route in &snapshot.http_routes {
            let key = namespaced_key(route.namespace.as_str(), route.name.as_str());
            let runtime_id =
                stable_runtime_id(&["http-route", route.namespace.as_str(), route.name.as_str()]);
            index.http_routes.insert(key.clone(), runtime_id);
            index.insert_resource_ref(
                runtime_id,
                RuntimeResourceRef::HttpRoute {
                    namespace: route.namespace.clone(),
                    name: route.name.clone(),
                },
            );
            for (rule_index, _) in route.rules.iter().enumerate() {
                let rule_index_text = rule_index.to_string();
                let runtime_id = stable_runtime_id(&[
                    "http-rule",
                    route.namespace.as_str(),
                    route.name.as_str(),
                    rule_index_text.as_str(),
                ]);
                index
                    .http_rules
                    .insert(rule_key(&key, rule_index), runtime_id);
                index.insert_resource_ref(
                    runtime_id,
                    RuntimeResourceRef::HttpRule {
                        namespace: route.namespace.clone(),
                        name: route.name.clone(),
                        rule_index,
                    },
                );
            }
        }

        for route in &snapshot.grpc_routes {
            let key = namespaced_key(route.namespace.as_str(), route.name.as_str());
            let runtime_id =
                stable_runtime_id(&["grpc-route", route.namespace.as_str(), route.name.as_str()]);
            index.grpc_routes.insert(key.clone(), runtime_id);
            index.insert_resource_ref(
                runtime_id,
                RuntimeResourceRef::GrpcRoute {
                    namespace: route.namespace.clone(),
                    name: route.name.clone(),
                },
            );
            for (rule_index, _) in route.rules.iter().enumerate() {
                let rule_index_text = rule_index.to_string();
                let runtime_id = stable_runtime_id(&[
                    "grpc-rule",
                    route.namespace.as_str(),
                    route.name.as_str(),
                    rule_index_text.as_str(),
                ]);
                index
                    .grpc_rules
                    .insert(rule_key(&key, rule_index), runtime_id);
                index.insert_resource_ref(
                    runtime_id,
                    RuntimeResourceRef::GrpcRule {
                        namespace: route.namespace.clone(),
                        name: route.name.clone(),
                        rule_index,
                    },
                );
            }
        }

        for route in &snapshot.stream_routes {
            let canonical_kind = canonical_stream_route_kind(route.kind.as_str()).to_string();
            let key = stream_route_key(
                route.kind.as_str(),
                route.namespace.as_str(),
                route.name.as_str(),
            );
            let runtime_id = stable_runtime_id(&[
                "stream-route",
                route.kind.as_str(),
                route.namespace.as_str(),
                route.name.as_str(),
            ]);
            index.stream_routes.insert(key.clone(), runtime_id);
            index.insert_resource_ref(
                runtime_id,
                RuntimeResourceRef::StreamRoute {
                    kind: canonical_kind.clone(),
                    namespace: route.namespace.clone(),
                    name: route.name.clone(),
                },
            );
            for (rule_index, _) in route.rules.iter().enumerate() {
                let rule_index_text = rule_index.to_string();
                let runtime_id = stable_runtime_id(&[
                    "stream-rule",
                    route.kind.as_str(),
                    route.namespace.as_str(),
                    route.name.as_str(),
                    rule_index_text.as_str(),
                ]);
                index
                    .stream_rules
                    .insert(rule_key(&key, rule_index), runtime_id);
                index.insert_resource_ref(
                    runtime_id,
                    RuntimeResourceRef::StreamRule {
                        kind: canonical_kind.clone(),
                        namespace: route.namespace.clone(),
                        name: route.name.clone(),
                        rule_index,
                    },
                );
            }
        }

        for backend in &snapshot.backends {
            let backend_name = namespaced_key(backend.namespace.as_ref(), backend.name.as_ref());
            let runtime_id = stable_runtime_id(&["backend", backend_name.as_str()]);
            index.backends.insert(backend_name.clone(), runtime_id);
            index.insert_resource_ref(
                runtime_id,
                RuntimeResourceRef::Backend {
                    name: backend_name.clone(),
                },
            );
            for endpoint in &backend.endpoints {
                let port = endpoint.port.to_string();
                let runtime_id = stable_runtime_id(&[
                    "endpoint",
                    backend_name.as_str(),
                    endpoint.address.as_str(),
                    port.as_str(),
                ]);
                index.endpoints.insert(
                    EndpointRuntimeKey::new(backend_name.as_str(), endpoint),
                    runtime_id,
                );
                index.insert_resource_ref(
                    runtime_id,
                    RuntimeResourceRef::Endpoint {
                        backend_name: backend_name.clone(),
                        address: endpoint.address.clone(),
                        port: endpoint.port,
                    },
                );
            }
        }

        index
    }

    fn insert_resource_ref(&mut self, runtime_id: RuntimeId, resource_ref: RuntimeResourceRef) {
        match self.resource_refs.entry(runtime_id) {
            Entry::Vacant(entry) => {
                entry.insert(resource_ref);
            }
            Entry::Occupied(entry) => {
                debug_assert_eq!(entry.get(), &resource_ref);
            }
        }
    }
}

fn namespaced_key(namespace: &str, name: &str) -> String {
    format!("{namespace}/{name}")
}

fn stream_route_key(kind: &str, namespace: &str, name: &str) -> String {
    let kind = canonical_stream_route_kind(kind);
    format!("{kind}/{namespace}/{name}")
}

fn canonical_stream_route_kind(kind: &str) -> &str {
    match kind {
        "ROUTE_KIND_TCP" | "TCP" | "TCPRoute" => "TCPRoute",
        "ROUTE_KIND_UDP" | "UDP" | "UDPRoute" => "UDPRoute",
        "ROUTE_KIND_TLS" | "TLS" | "TLSRoute" => "TLSRoute",
        _ => kind,
    }
}

fn rule_key(route_key: &str, rule_index: usize) -> String {
    format!("{route_key}#rule-{rule_index}")
}

fn stable_runtime_id(parts: &[&str]) -> RuntimeId {
    let mut hash = FNV_OFFSET_BASIS;
    for part in parts {
        hash = write_bytes(hash, &(part.len() as u64).to_le_bytes());
        hash = write_bytes(hash, part.as_bytes());
    }

    RuntimeId(hash.max(1))
}

fn write_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

impl crate::selection::RouteKind {
    fn runtime_id_key(self) -> &'static str {
        match self {
            Self::Http => "http-route",
            Self::Grpc => "grpc-route",
            Self::Tcp => "TCPRoute",
            Self::Udp => "UDPRoute",
            Self::Tls => "TLSRoute",
        }
    }
}

trait SelectedBackendRuntimeIdExt {
    fn route_kind_runtime_id(&self, snapshot: &Snapshot) -> Option<RuntimeId>;
    fn rule_kind_runtime_id(&self, snapshot: &Snapshot) -> Option<RuntimeId>;
}

impl SelectedBackendRuntimeIdExt for crate::SelectedBackend {
    fn route_kind_runtime_id(&self, snapshot: &Snapshot) -> Option<RuntimeId> {
        match self.route_kind {
            crate::selection::RouteKind::Http => snapshot
                .http_route_runtime_id(self.route_namespace.as_str(), self.route_name.as_str()),
            crate::selection::RouteKind::Grpc => snapshot
                .grpc_route_runtime_id(self.route_namespace.as_str(), self.route_name.as_str()),
            crate::selection::RouteKind::Tcp
            | crate::selection::RouteKind::Udp
            | crate::selection::RouteKind::Tls => snapshot.stream_route_runtime_id(
                self.route_kind.runtime_id_key(),
                self.route_namespace.as_str(),
                self.route_name.as_str(),
            ),
        }
    }

    fn rule_kind_runtime_id(&self, snapshot: &Snapshot) -> Option<RuntimeId> {
        let rule_index = self.rule_index?;
        match self.route_kind {
            crate::selection::RouteKind::Http => snapshot.http_rule_runtime_id(
                self.route_namespace.as_str(),
                self.route_name.as_str(),
                rule_index,
            ),
            crate::selection::RouteKind::Grpc => snapshot.grpc_rule_runtime_id(
                self.route_namespace.as_str(),
                self.route_name.as_str(),
                rule_index,
            ),
            crate::selection::RouteKind::Tcp
            | crate::selection::RouteKind::Udp
            | crate::selection::RouteKind::Tls => snapshot.stream_rule_runtime_id(
                self.route_kind.runtime_id_key(),
                self.route_namespace.as_str(),
                self.route_name.as_str(),
                rule_index,
            ),
        }
    }
}
