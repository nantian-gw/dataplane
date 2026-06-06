use super::filters::filter_from_proto;
use super::values::duration_from_proto;
use super::*;

pub(super) fn tls_from_proto(item: proto::TlsConfig) -> TlsConfig {
    TlsConfig {
        enabled: item.enabled,
        passthrough: item.passthrough,
        secret_refs: item.secret_refs,
        sni_hosts: item.sni_hosts,
        min_version: item.min_version,
        max_version: item.max_version,
        frontend_validation: item.frontend_validation.map(frontend_validation_from_proto),
    }
}

fn frontend_validation_from_proto(item: proto::FrontendValidation) -> crate::FrontendValidation {
    crate::FrontendValidation {
        ca_pems: item.ca_pems,
        mode: item.mode,
    }
}

pub(super) fn backend_tls_from_proto(item: proto::BackendTlsConfig) -> crate::BackendTlsConfig {
    crate::BackendTlsConfig {
        client_certificate_ref: item.client_certificate_ref,
    }
}

pub(super) fn http_rule_from_proto(rule: proto::HttpRule) -> HttpRule {
    HttpRule {
        name: rule.name,
        matches: rule
            .matches
            .into_iter()
            .map(|item| HttpMatch {
                path: item.path,
                path_type: item.path_type,
                method: item.method,
                headers: item.headers.into_iter().map(header_from_proto).collect(),
                query_params: item
                    .query_params
                    .into_iter()
                    .map(query_from_proto)
                    .collect(),
                ..HttpMatch::default()
            })
            .collect(),
        filters: rule.filters.into_iter().map(filter_from_proto).collect(),
        backend_refs: rule
            .backend_refs
            .into_iter()
            .map(backend_ref_from_proto)
            .collect(),
        timeouts: rule.timeouts.map(route_timeouts_from_proto),
        retry: rule.retry.map(retry_policy_from_proto),
        session_persistence: rule.session_persistence.map(session_persistence_from_proto),
    }
}

pub(super) fn grpc_rule_from_proto(rule: proto::GrpcRule) -> GrpcRule {
    GrpcRule {
        name: rule.name,
        matches: rule
            .matches
            .into_iter()
            .map(|item| GrpcMatch {
                service: item.service,
                method: item.method,
                match_type: item.match_type,
                headers: item.headers.into_iter().map(header_from_proto).collect(),
                ..GrpcMatch::default()
            })
            .collect(),
        filters: rule.filters.into_iter().map(filter_from_proto).collect(),
        backend_refs: rule
            .backend_refs
            .into_iter()
            .map(backend_ref_from_proto)
            .collect(),
        session_persistence: rule.session_persistence.map(session_persistence_from_proto),
    }
}

pub(super) fn stream_rule_from_proto(rule: proto::StreamRule) -> StreamRule {
    StreamRule {
        name: rule.name,
        matches: rule
            .matches
            .into_iter()
            .map(|item| {
                let mode = match item.mode() {
                    proto::TlsRouteMode::Terminate => crate::TlsRouteMode::Terminate,
                    _ => crate::TlsRouteMode::Passthrough,
                };
                StreamMatch {
                    port: item.port,
                    sni_hostname: item.sni_hostname,
                    mode,
                }
            })
            .collect(),
        backend_refs: rule
            .backend_refs
            .into_iter()
            .map(backend_ref_from_proto)
            .collect(),
    }
}

pub(super) fn backend_ref_from_proto(item: proto::BackendRef) -> BackendRef {
    BackendRef {
        group: item.group,
        kind: item.kind,
        namespace: item.namespace,
        name: item.name,
        port: item.port,
        weight: item.weight,
        metadata: item.metadata.into_iter().collect(),
        filters: item.filters.into_iter().map(filter_from_proto).collect(),
    }
}

pub(super) fn parent_ref_from_proto(item: proto::ParentRef) -> ParentRef {
    ParentRef {
        group: item.group,
        kind: item.kind,
        namespace: item.namespace,
        name: item.name,
        section_name: item.section_name,
        port: item.port,
    }
}

fn route_timeouts_from_proto(item: proto::HttpRouteTimeouts) -> RouteTimeouts {
    RouteTimeouts {
        request: item.request.as_ref().and_then(duration_from_proto),
        backend_request: item.backend_request.as_ref().and_then(duration_from_proto),
    }
}

fn retry_policy_from_proto(item: proto::HttpRouteRetry) -> RetryPolicy {
    RetryPolicy {
        codes: item.codes,
        attempts: item.attempts,
        backoff: item.backoff.as_ref().and_then(duration_from_proto),
    }
}

pub(super) fn session_persistence_from_proto(
    item: proto::SessionPersistence,
) -> SessionPersistence {
    let session_type = session_type_from_proto(item.r#type());
    SessionPersistence {
        session_name: item.session_name,
        session_type,
        absolute_timeout: item.absolute_timeout.as_ref().and_then(duration_from_proto),
        idle_timeout: item.idle_timeout.as_ref().and_then(duration_from_proto),
        cookie: item.cookie.map(cookie_config_from_proto),
    }
}

fn cookie_config_from_proto(item: proto::CookieConfig) -> crate::CookieConfig {
    crate::CookieConfig {
        lifetime_type: cookie_lifetime_type_from_proto(item.lifetime_type()),
    }
}

fn session_type_from_proto(item: proto::SessionPersistenceType) -> String {
    match item {
        proto::SessionPersistenceType::Header => "Header".to_string(),
        proto::SessionPersistenceType::Cookie | proto::SessionPersistenceType::Unspecified => {
            "Cookie".to_string()
        }
    }
}

fn cookie_lifetime_type_from_proto(item: proto::CookieLifetimeType) -> String {
    match item {
        proto::CookieLifetimeType::Permanent => "Permanent".to_string(),
        proto::CookieLifetimeType::Session | proto::CookieLifetimeType::Unspecified => {
            "Session".to_string()
        }
    }
}

fn header_from_proto(item: proto::HeaderMatch) -> HeaderMatch {
    HeaderMatch {
        name: item.name,
        value: item.value,
        match_type: item.match_type,
        ..HeaderMatch::default()
    }
}

fn query_from_proto(item: proto::QueryMatch) -> QueryMatch {
    QueryMatch {
        name: item.name,
        value: item.value,
        match_type: item.match_type,
        ..QueryMatch::default()
    }
}
