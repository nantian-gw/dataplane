use aeg_ir::{
    BackendCluster, GrpcRoute, HttpRoute, Listener, RuntimeId, RuntimeResourceRef, Snapshot,
    StreamRoute,
};
use serde::Serialize;
use serde_json::Value;

use super::types::{
    ApiError, BackendListQuery, ListenerListQuery, RouteListQuery, RouteListResponse,
    RouteListValueResponse,
};

pub(super) fn filter_listeners(
    snapshot: &Snapshot,
    query: &ListenerListQuery,
) -> Result<Vec<Listener>, ApiError> {
    let protocol = parse_protocol_filter(query.protocol.as_deref())?;
    let name = trim_option(query.name.as_deref());
    let hostname = trim_option(query.hostname.as_deref());
    let attached_route = trim_option(query.attached_route.as_deref());
    let runtime_ref = parse_runtime_ref_filter(snapshot, query.runtime_id.as_deref(), "runtimeId")?;

    Ok(snapshot
        .listeners
        .iter()
        .filter(|listener| listener_runtime_ref_matches(&runtime_ref, listener))
        .filter(|listener| match name {
            Some(name) => listener.name == name,
            None => true,
        })
        .filter(|listener| match protocol.as_deref() {
            Some(protocol) => canonical_protocol(&listener.protocol) == Some(protocol.to_string()),
            None => true,
        })
        .filter(|listener| match hostname {
            Some(hostname) => listener.hostnames.iter().any(|value| value == hostname),
            None => true,
        })
        .filter(|listener| match attached_route {
            Some(attached_route) => listener
                .attached_routes
                .iter()
                .any(|value| value == attached_route),
            None => true,
        })
        .cloned()
        .collect())
}

pub(super) fn find_listener(snapshot: &Snapshot, name: &str) -> Option<Listener> {
    snapshot
        .listeners
        .iter()
        .find(|listener| listener.name == name)
        .cloned()
}

pub(super) fn listener_list_values(
    snapshot: &Snapshot,
    query: &ListenerListQuery,
) -> Result<Vec<Value>, ApiError> {
    filter_listeners(snapshot, query)?
        .iter()
        .map(|listener| listener_detail_value(snapshot, listener))
        .collect()
}

pub(super) fn listener_detail_value(
    snapshot: &Snapshot,
    listener: &Listener,
) -> Result<Value, ApiError> {
    let mut value = resource_to_value(listener)?;
    insert_runtime_identity(
        &mut value,
        snapshot,
        snapshot.listener_runtime_id(listener.name.as_str()),
    )?;
    Ok(value)
}

pub(super) fn filter_routes(
    snapshot: &Snapshot,
    query: &RouteListQuery,
) -> Result<RouteListResponse, ApiError> {
    let kind = parse_route_kind_filter(query.kind.as_deref())?;
    let namespace = trim_option(query.namespace.as_deref());
    let name = trim_option(query.name.as_deref());
    let hostname = trim_option(query.hostname.as_deref());
    let runtime_ref = parse_runtime_ref_filter(snapshot, query.runtime_id.as_deref(), "runtimeId")?;
    let rule_runtime_ref =
        parse_runtime_ref_filter(snapshot, query.rule_runtime_id.as_deref(), "ruleRuntimeId")?;

    let mut response = RouteListResponse::default();
    if kind.is_none() || kind.as_deref() == Some("HTTP") {
        response.http = snapshot
            .http_routes
            .iter()
            .filter(|route| http_route_matches(route, namespace, name, hostname))
            .filter(|route| http_route_runtime_ref_matches(&runtime_ref, route))
            .filter(|route| http_rule_runtime_ref_matches(&rule_runtime_ref, route))
            .cloned()
            .collect();
    }
    if kind.is_none() || kind.as_deref() == Some("GRPC") {
        response.grpc = snapshot
            .grpc_routes
            .iter()
            .filter(|route| grpc_route_matches(route, namespace, name, hostname))
            .filter(|route| grpc_route_runtime_ref_matches(&runtime_ref, route))
            .filter(|route| grpc_rule_runtime_ref_matches(&rule_runtime_ref, route))
            .cloned()
            .collect();
    }
    if kind.is_none() || matches!(kind.as_deref(), Some("TCP") | Some("UDP") | Some("TLS")) {
        response.stream = snapshot
            .stream_routes
            .iter()
            .filter(|route| match kind.as_deref() {
                Some(kind) => canonical_route_kind(&route.kind).as_deref() == Some(kind),
                None => true,
            })
            .filter(|route| stream_route_matches(route, namespace, name, hostname))
            .filter(|route| stream_route_runtime_ref_matches(&runtime_ref, route))
            .filter(|route| stream_rule_runtime_ref_matches(&rule_runtime_ref, route))
            .cloned()
            .collect();
    }

    Ok(response)
}

pub(super) fn route_list_values(
    snapshot: &Snapshot,
    query: &RouteListQuery,
) -> Result<RouteListValueResponse, ApiError> {
    let routes = filter_routes(snapshot, query)?;
    Ok(RouteListValueResponse {
        http: routes
            .http
            .iter()
            .map(|route| http_route_detail_value(snapshot, route))
            .collect::<Result<Vec<_>, _>>()?,
        grpc: routes
            .grpc
            .iter()
            .map(|route| grpc_route_detail_value(snapshot, route))
            .collect::<Result<Vec<_>, _>>()?,
        stream: routes
            .stream
            .iter()
            .map(|route| stream_route_detail_value(snapshot, route))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub(super) fn find_route(
    snapshot: &Snapshot,
    kind: &str,
    namespace: &str,
    name: &str,
) -> Result<Option<Value>, ApiError> {
    let kind = parse_required_route_kind(kind)?;
    let value = match kind.as_str() {
        "HTTP" => snapshot
            .http_routes
            .iter()
            .find(|route| route.namespace == namespace && route.name == name)
            .map(|route| http_route_detail_value(snapshot, route)),
        "GRPC" => snapshot
            .grpc_routes
            .iter()
            .find(|route| route.namespace == namespace && route.name == name)
            .map(|route| grpc_route_detail_value(snapshot, route)),
        _ => snapshot
            .stream_routes
            .iter()
            .find(|route| {
                canonical_route_kind(&route.kind).as_deref() == Some(kind.as_str())
                    && route.namespace == namespace
                    && route.name == name
            })
            .map(|route| stream_route_detail_value(snapshot, route)),
    };

    value.transpose()
}

pub(super) fn filter_backends(
    snapshot: &Snapshot,
    query: &BackendListQuery,
) -> Result<Vec<BackendCluster>, ApiError> {
    let protocol = parse_backend_protocol_filter(query.protocol.as_deref())?;
    let namespace = trim_option(query.namespace.as_deref());
    let name = trim_option(query.name.as_deref());
    let runtime_ref = parse_runtime_ref_filter(snapshot, query.runtime_id.as_deref(), "runtimeId")?;
    let endpoint_runtime_ref = parse_runtime_ref_filter(
        snapshot,
        query.endpoint_runtime_id.as_deref(),
        "endpointRuntimeId",
    )?;

    Ok(snapshot
        .backends
        .iter()
        .filter(|backend| backend_runtime_ref_matches(&runtime_ref, backend))
        .filter(|backend| endpoint_runtime_ref_matches(&endpoint_runtime_ref, backend))
        .filter(|backend| match namespace {
            Some(namespace) => backend.namespace == namespace,
            None => true,
        })
        .filter(|backend| match name {
            Some(name) => backend.name == name,
            None => true,
        })
        .filter(|backend| match protocol.as_deref() {
            Some(protocol) => {
                canonical_backend_protocol(&backend.protocol) == Some(protocol.to_string())
            }
            None => true,
        })
        .cloned()
        .collect())
}

pub(super) fn find_backend(
    snapshot: &Snapshot,
    namespace: &str,
    name: &str,
) -> Option<BackendCluster> {
    snapshot
        .backends
        .iter()
        .find(|backend| backend.namespace == namespace && backend.name == name)
        .cloned()
}

pub(super) fn backend_list_values(
    snapshot: &Snapshot,
    query: &BackendListQuery,
) -> Result<Vec<Value>, ApiError> {
    filter_backends(snapshot, query)?
        .iter()
        .map(|backend| backend_detail_value(snapshot, backend))
        .collect()
}

pub(super) fn backend_detail_value(
    snapshot: &Snapshot,
    backend: &BackendCluster,
) -> Result<Value, ApiError> {
    let mut value = resource_to_value(backend)?;
    let backend_key = backend_runtime_key(backend);
    insert_runtime_identity(
        &mut value,
        snapshot,
        snapshot.backend_runtime_id(&backend_key),
    )?;
    insert_endpoint_runtime_ids(&mut value, snapshot, &backend_key, backend)?;
    Ok(value)
}

fn http_route_detail_value(snapshot: &Snapshot, route: &HttpRoute) -> Result<Value, ApiError> {
    let mut value = resource_to_value(route)?;
    insert_runtime_identity(
        &mut value,
        snapshot,
        snapshot.http_route_runtime_id(route.namespace.as_str(), route.name.as_str()),
    )?;
    insert_rule_runtime_ids(
        &mut value,
        snapshot,
        (0..route.rules.len())
            .map(|index| snapshot.http_rule_runtime_id(&route.namespace, &route.name, index)),
    )?;
    Ok(value)
}

fn grpc_route_detail_value(snapshot: &Snapshot, route: &GrpcRoute) -> Result<Value, ApiError> {
    let mut value = resource_to_value(route)?;
    insert_runtime_identity(
        &mut value,
        snapshot,
        snapshot.grpc_route_runtime_id(route.namespace.as_str(), route.name.as_str()),
    )?;
    insert_rule_runtime_ids(
        &mut value,
        snapshot,
        (0..route.rules.len())
            .map(|index| snapshot.grpc_rule_runtime_id(&route.namespace, &route.name, index)),
    )?;
    Ok(value)
}

fn stream_route_detail_value(snapshot: &Snapshot, route: &StreamRoute) -> Result<Value, ApiError> {
    let mut value = resource_to_value(route)?;
    insert_runtime_identity(
        &mut value,
        snapshot,
        snapshot.stream_route_runtime_id(
            route.kind.as_str(),
            route.namespace.as_str(),
            route.name.as_str(),
        ),
    )?;
    insert_rule_runtime_ids(
        &mut value,
        snapshot,
        (0..route.rules.len()).map(|index| {
            snapshot.stream_rule_runtime_id(&route.kind, &route.namespace, &route.name, index)
        }),
    )?;
    Ok(value)
}

fn resource_to_value<T>(resource: &T) -> Result<Value, ApiError>
where
    T: Serialize,
{
    serde_json::to_value(resource).map_err(|err| ApiError::internal(err.to_string()))
}

fn insert_runtime_identity(
    value: &mut Value,
    snapshot: &Snapshot,
    runtime_id: Option<RuntimeId>,
) -> Result<(), ApiError> {
    if let Some(runtime_id) = runtime_id {
        let object = object_mut(value)?;
        object.insert(
            "runtimeId".to_string(),
            Value::String(runtime_id.to_string()),
        );
        if let Some(resource_ref) = snapshot.runtime_resource_ref(runtime_id) {
            object.insert("runtimeRef".to_string(), runtime_ref_value(resource_ref));
        }
    }
    Ok(())
}

fn insert_rule_runtime_ids(
    value: &mut Value,
    snapshot: &Snapshot,
    runtime_ids: impl Iterator<Item = Option<RuntimeId>>,
) -> Result<(), ApiError> {
    let pairs: Vec<(Value, Value)> = runtime_ids
        .map(|id| {
            let id_value = id
                .map(|id| Value::String(id.to_string()))
                .unwrap_or(Value::Null);
            let ref_value = id
                .and_then(|id| snapshot.runtime_resource_ref(id))
                .map(runtime_ref_value)
                .unwrap_or(Value::Null);
            (id_value, ref_value)
        })
        .collect();
    let ids: Vec<Value> = pairs.iter().map(|(id, _)| id.clone()).collect();
    if ids.iter().any(|id| !id.is_null()) {
        let refs: Vec<Value> = pairs
            .into_iter()
            .map(|(_, resource_ref)| resource_ref)
            .collect();
        let object = object_mut(value)?;
        object.insert("ruleRuntimeIds".to_string(), Value::Array(ids));
        object.insert("ruleRuntimeRefs".to_string(), Value::Array(refs));
    }
    Ok(())
}

fn insert_endpoint_runtime_ids(
    value: &mut Value,
    snapshot: &Snapshot,
    backend_key: &str,
    backend: &BackendCluster,
) -> Result<(), ApiError> {
    let endpoints = value
        .get_mut("endpoints")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| ApiError::internal("backend endpoints did not serialize as an array"))?;

    for (index, endpoint) in backend.endpoints.iter().enumerate() {
        let Some(runtime_id) = snapshot.endpoint_runtime_id(backend_key, endpoint) else {
            continue;
        };
        if let Some(Value::Object(endpoint_value)) = endpoints.get_mut(index) {
            endpoint_value.insert(
                "runtimeId".to_string(),
                Value::String(runtime_id.to_string()),
            );
            if let Some(resource_ref) = snapshot.runtime_resource_ref(runtime_id) {
                endpoint_value.insert("runtimeRef".to_string(), runtime_ref_value(resource_ref));
            }
        }
    }
    Ok(())
}

fn backend_runtime_key(backend: &BackendCluster) -> String {
    format!("{}/{}", backend.namespace, backend.name)
}

pub(super) fn runtime_ref_value(resource_ref: RuntimeResourceRef) -> Value {
    match resource_ref {
        RuntimeResourceRef::Listener { name } => {
            serde_json::json!({"kind": "Listener", "name": name})
        }
        RuntimeResourceRef::HttpRoute { namespace, name } => {
            serde_json::json!({"kind": "HTTPRoute", "namespace": namespace, "name": name})
        }
        RuntimeResourceRef::GrpcRoute { namespace, name } => {
            serde_json::json!({"kind": "GRPCRoute", "namespace": namespace, "name": name})
        }
        RuntimeResourceRef::StreamRoute {
            kind,
            namespace,
            name,
        } => serde_json::json!({"kind": kind, "namespace": namespace, "name": name}),
        RuntimeResourceRef::HttpRule {
            namespace,
            name,
            rule_index,
        } => serde_json::json!({
            "kind": "HTTPRouteRule",
            "namespace": namespace,
            "name": name,
            "ruleIndex": rule_index,
        }),
        RuntimeResourceRef::GrpcRule {
            namespace,
            name,
            rule_index,
        } => serde_json::json!({
            "kind": "GRPCRouteRule",
            "namespace": namespace,
            "name": name,
            "ruleIndex": rule_index,
        }),
        RuntimeResourceRef::StreamRule {
            kind,
            namespace,
            name,
            rule_index,
        } => serde_json::json!({
            "kind": format!("{kind}Rule"),
            "namespace": namespace,
            "name": name,
            "ruleIndex": rule_index,
        }),
        RuntimeResourceRef::Backend { name } => {
            serde_json::json!({"kind": "Backend", "name": name})
        }
        RuntimeResourceRef::Endpoint {
            backend_name,
            address,
            port,
        } => serde_json::json!({
            "kind": "Endpoint",
            "backendName": backend_name,
            "address": address,
            "port": port,
        }),
    }
}

enum RuntimeRefFilter {
    Any,
    Resource(RuntimeResourceRef),
    NoMatch,
}

fn parse_runtime_ref_filter(
    snapshot: &Snapshot,
    raw: Option<&str>,
    field_name: &str,
) -> Result<RuntimeRefFilter, ApiError> {
    let Some(value) = trim_option(raw) else {
        return Ok(RuntimeRefFilter::Any);
    };
    let runtime_id = RuntimeId::parse_hex(value).ok_or_else(|| {
        ApiError::bad_request(format!(
            "{field_name} must be a 16-character hex runtime ID"
        ))
    })?;
    Ok(snapshot
        .runtime_resource_ref(runtime_id)
        .map(RuntimeRefFilter::Resource)
        .unwrap_or(RuntimeRefFilter::NoMatch))
}

fn listener_runtime_ref_matches(filter: &RuntimeRefFilter, listener: &Listener) -> bool {
    match filter {
        RuntimeRefFilter::Any => true,
        RuntimeRefFilter::NoMatch => false,
        RuntimeRefFilter::Resource(RuntimeResourceRef::Listener { name }) => {
            listener.name.as_str() == name.as_str()
        }
        RuntimeRefFilter::Resource(_) => false,
    }
}

fn http_route_runtime_ref_matches(filter: &RuntimeRefFilter, route: &HttpRoute) -> bool {
    match filter {
        RuntimeRefFilter::Any => true,
        RuntimeRefFilter::NoMatch => false,
        RuntimeRefFilter::Resource(RuntimeResourceRef::HttpRoute { namespace, name }) => {
            route.namespace.as_str() == namespace.as_str() && route.name.as_str() == name.as_str()
        }
        RuntimeRefFilter::Resource(_) => false,
    }
}

fn http_rule_runtime_ref_matches(filter: &RuntimeRefFilter, route: &HttpRoute) -> bool {
    match filter {
        RuntimeRefFilter::Any => true,
        RuntimeRefFilter::NoMatch => false,
        RuntimeRefFilter::Resource(RuntimeResourceRef::HttpRule {
            namespace, name, ..
        }) => {
            route.namespace.as_str() == namespace.as_str() && route.name.as_str() == name.as_str()
        }
        RuntimeRefFilter::Resource(_) => false,
    }
}

fn grpc_route_runtime_ref_matches(filter: &RuntimeRefFilter, route: &GrpcRoute) -> bool {
    match filter {
        RuntimeRefFilter::Any => true,
        RuntimeRefFilter::NoMatch => false,
        RuntimeRefFilter::Resource(RuntimeResourceRef::GrpcRoute { namespace, name }) => {
            route.namespace.as_str() == namespace.as_str() && route.name.as_str() == name.as_str()
        }
        RuntimeRefFilter::Resource(_) => false,
    }
}

fn grpc_rule_runtime_ref_matches(filter: &RuntimeRefFilter, route: &GrpcRoute) -> bool {
    match filter {
        RuntimeRefFilter::Any => true,
        RuntimeRefFilter::NoMatch => false,
        RuntimeRefFilter::Resource(RuntimeResourceRef::GrpcRule {
            namespace, name, ..
        }) => {
            route.namespace.as_str() == namespace.as_str() && route.name.as_str() == name.as_str()
        }
        RuntimeRefFilter::Resource(_) => false,
    }
}

fn stream_route_runtime_ref_matches(filter: &RuntimeRefFilter, route: &StreamRoute) -> bool {
    match filter {
        RuntimeRefFilter::Any => true,
        RuntimeRefFilter::NoMatch => false,
        RuntimeRefFilter::Resource(RuntimeResourceRef::StreamRoute {
            kind,
            namespace,
            name,
        }) => {
            canonical_route_kind(kind).as_deref() == canonical_route_kind(&route.kind).as_deref()
                && route.namespace.as_str() == namespace.as_str()
                && route.name.as_str() == name.as_str()
        }
        RuntimeRefFilter::Resource(_) => false,
    }
}

fn stream_rule_runtime_ref_matches(filter: &RuntimeRefFilter, route: &StreamRoute) -> bool {
    match filter {
        RuntimeRefFilter::Any => true,
        RuntimeRefFilter::NoMatch => false,
        RuntimeRefFilter::Resource(RuntimeResourceRef::StreamRule {
            kind,
            namespace,
            name,
            ..
        }) => {
            canonical_route_kind(kind).as_deref() == canonical_route_kind(&route.kind).as_deref()
                && route.namespace.as_str() == namespace.as_str()
                && route.name.as_str() == name.as_str()
        }
        RuntimeRefFilter::Resource(_) => false,
    }
}

fn backend_runtime_ref_matches(filter: &RuntimeRefFilter, backend: &BackendCluster) -> bool {
    match filter {
        RuntimeRefFilter::Any => true,
        RuntimeRefFilter::NoMatch => false,
        RuntimeRefFilter::Resource(RuntimeResourceRef::Backend { name }) => {
            backend_runtime_key(backend) == name.as_str()
        }
        RuntimeRefFilter::Resource(_) => false,
    }
}

fn endpoint_runtime_ref_matches(filter: &RuntimeRefFilter, backend: &BackendCluster) -> bool {
    match filter {
        RuntimeRefFilter::Any => true,
        RuntimeRefFilter::NoMatch => false,
        RuntimeRefFilter::Resource(RuntimeResourceRef::Endpoint { backend_name, .. }) => {
            backend_runtime_key(backend) == backend_name.as_str()
        }
        RuntimeRefFilter::Resource(_) => false,
    }
}

fn object_mut(value: &mut Value) -> Result<&mut serde_json::Map<String, Value>, ApiError> {
    value
        .as_object_mut()
        .ok_or_else(|| ApiError::internal("admin resource did not serialize as an object"))
}

fn http_route_matches(
    route: &HttpRoute,
    namespace: Option<&str>,
    name: Option<&str>,
    hostname: Option<&str>,
) -> bool {
    namespace
        .map(|value| route.namespace == value)
        .unwrap_or(true)
        && name.map(|value| route.name == value).unwrap_or(true)
        && hostname
            .map(|value| route.hostnames.iter().any(|item| item == value))
            .unwrap_or(true)
}

fn grpc_route_matches(
    route: &GrpcRoute,
    namespace: Option<&str>,
    name: Option<&str>,
    hostname: Option<&str>,
) -> bool {
    namespace
        .map(|value| route.namespace == value)
        .unwrap_or(true)
        && name.map(|value| route.name == value).unwrap_or(true)
        && hostname
            .map(|value| route.hostnames.iter().any(|item| item == value))
            .unwrap_or(true)
}

fn stream_route_matches(
    route: &StreamRoute,
    namespace: Option<&str>,
    name: Option<&str>,
    hostname: Option<&str>,
) -> bool {
    namespace
        .map(|value| route.namespace == value)
        .unwrap_or(true)
        && name.map(|value| route.name == value).unwrap_or(true)
        && hostname
            .map(|value| {
                route.rules.iter().any(|rule| {
                    rule.matches
                        .iter()
                        .any(|matched| matched.sni_hostname == value)
                })
            })
            .unwrap_or(true)
}

fn parse_protocol_filter(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    parse_canonical_token(raw, canonical_protocol, "invalid protocol")
}

fn parse_backend_protocol_filter(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    parse_canonical_token(raw, canonical_backend_protocol, "invalid backend protocol")
}

fn parse_route_kind_filter(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    parse_canonical_token(raw, canonical_route_kind, "invalid route kind")
}

fn parse_required_route_kind(raw: &str) -> Result<String, ApiError> {
    parse_route_kind_filter(Some(raw))?
        .ok_or_else(|| ApiError::bad_request("route kind is required"))
}

fn parse_canonical_token(
    raw: Option<&str>,
    normalizer: fn(&str) -> Option<String>,
    message: &'static str,
) -> Result<Option<String>, ApiError> {
    match trim_option(raw) {
        Some(value) => normalizer(value)
            .map(Some)
            .ok_or_else(|| ApiError::bad_request(message)),
        None => Ok(None),
    }
}

fn canonical_protocol(protocol: &str) -> Option<String> {
    match normalize_token(protocol).as_str() {
        "HTTP" => Some("HTTP".to_string()),
        "HTTPS" => Some("HTTPS".to_string()),
        "GRPC" => Some("GRPC".to_string()),
        "HTTP3" => Some("HTTP3".to_string()),
        "TCP" => Some("TCP".to_string()),
        "UDP" => Some("UDP".to_string()),
        "TLS" | "TLSPASSTHROUGH" => Some("TLS".to_string()),
        _ => None,
    }
}

fn canonical_backend_protocol(protocol: &str) -> Option<String> {
    match normalize_token(protocol).as_str() {
        "HTTP" => Some("HTTP".to_string()),
        "HTTPS" => Some("HTTPS".to_string()),
        "GRPC" => Some("GRPC".to_string()),
        "GRPCS" => Some("GRPCS".to_string()),
        "H2C" => Some("H2C".to_string()),
        "TCP" => Some("TCP".to_string()),
        "UDP" => Some("UDP".to_string()),
        "" => None,
        other => Some(other.to_string()),
    }
}

fn canonical_route_kind(kind: &str) -> Option<String> {
    match normalize_token(kind).as_str() {
        "HTTP" => Some("HTTP".to_string()),
        "GRPC" => Some("GRPC".to_string()),
        "TCP" => Some("TCP".to_string()),
        "UDP" => Some("UDP".to_string()),
        "TLS" => Some("TLS".to_string()),
        _ => None,
    }
}

fn normalize_token(value: &str) -> String {
    let mut value = value.trim().to_uppercase();
    value = value.replace('-', "_");
    if let Some(stripped) = value.strip_prefix("LISTENER_PROTOCOL_") {
        value = stripped.to_string();
    }
    if let Some(stripped) = value.strip_prefix("ROUTE_KIND_") {
        value = stripped.to_string();
    }
    value = value.replace('_', "");
    if let Some(stripped) = value.strip_suffix("ROUTE") {
        value = stripped.to_string();
    }
    value
}

fn trim_option(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub(super) fn is_http_listener(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_HTTP"
            | "LISTENER_PROTOCOL_HTTPS"
            | "LISTENER_PROTOCOL_HTTP3"
            | "LISTENER_PROTOCOL_GRPC"
            | "HTTP"
            | "HTTPS"
            | "HTTP3"
            | "GRPC"
    )
}

pub(super) fn is_plain_http_runtime_listener(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_HTTP"
            | "LISTENER_PROTOCOL_HTTP3"
            | "LISTENER_PROTOCOL_GRPC"
            | "HTTP"
            | "HTTP3"
            | "GRPC"
    )
}

pub(super) fn is_https_listener(protocol: &str) -> bool {
    matches!(protocol, "LISTENER_PROTOCOL_HTTPS" | "HTTPS")
}

pub(super) fn is_tls_runtime_listener(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_HTTPS"
            | "LISTENER_PROTOCOL_TLS"
            | "LISTENER_PROTOCOL_TLS_PASSTHROUGH"
            | "HTTPS"
            | "TLS"
            | "TLS_PASSTHROUGH"
    )
}

pub(super) fn is_pure_stream_runtime_listener(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_TCP" | "LISTENER_PROTOCOL_UDP" | "TCP" | "UDP"
    )
}

pub(super) fn is_stream_listener(protocol: &str) -> bool {
    matches!(
        protocol,
        "LISTENER_PROTOCOL_TCP"
            | "LISTENER_PROTOCOL_UDP"
            | "LISTENER_PROTOCOL_TLS"
            | "LISTENER_PROTOCOL_TLS_PASSTHROUGH"
            | "TCP"
            | "UDP"
            | "TLS"
            | "TLS_PASSTHROUGH"
    )
}
