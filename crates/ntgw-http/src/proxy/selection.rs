use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use ntgw_ir::{
    BackendPolicy, BackendSelectionError, RouteKind, SelectedBackend, SelectedBackendRuntimeIds,
    SelectedHttpRoute, Snapshot,
};

use super::{
    backend::{
        backend_tls_sni_name, effective_request_timeout_with_route_policy,
        is_http2_backend_protocol,
        is_tls_backend_protocol, resolve_backend_client_cert_key, resolve_backend_tls_validation,
    },
    context::{SelectedBackendConfig, UpstreamPeerAddress, route_kind_name},
};
use ntgw_observability::TrafficTopology;

#[derive(Debug, Default)]
pub(crate) struct SelectedBackendConfigCache;

#[derive(Debug, Default)]
struct SelectedBackendConfigCacheState {
    snapshot_id: String,
    entries: HashMap<SelectedBackendConfigCacheKey, Arc<SelectedBackendConfig>>,
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
struct SelectedBackendConfigCacheKey {
    route_kind: u8,
    listener: u64,
    route: u64,
    rule: u64,
    backend: u64,
    endpoint: u64,
}

thread_local! {
    static SELECTED_BACKEND_CONFIG_CACHE: RefCell<SelectedBackendConfigCacheState> =
        RefCell::new(SelectedBackendConfigCacheState::default());
}

pub(crate) fn selected_backend_from_http_route(
    route: SelectedHttpRoute,
    access_log_enabled: bool,
) -> Result<Option<SelectedBackend>, BackendSelectionError> {
    let SelectedHttpRoute {
        route_name,
        route_namespace,
        rule_index,
        route_annotations,
        listener_name,
        listener_protocol,
        filters,
        matched_http_path,
        backend,
        backend_name,
        backend_error,
        timeouts,
        retry,
        session_persistence,
        backend_tls,
        route_policy: _route_policy,
    } = route;

    match (backend, backend_name, backend_error) {
        (Some(backend), Some(backend_name), _) => {
            let route_annotations = if access_log_enabled {
                route_annotations
            } else {
                BTreeMap::new()
            };
            Ok(Some(SelectedBackend {
                route_kind: RouteKind::Http,
                route_name,
                route_namespace,
                rule_index,
                route_annotations,
                listener_name,
                listener_protocol,
                backend,
                backend_name,
                filters,
                matched_http_path: Some(matched_http_path),
                timeouts,
                retry,
                session_persistence,
                backend_tls,
                route_policy: _route_policy,
            }))
        }
        (_, _, Some(err)) => Err(err),
        _ => Ok(None),
    }
}

#[cfg(test)]
pub(crate) fn selected_backend_config(
    current: &Snapshot,
    selected: &SelectedBackend,
) -> pingora::Result<SelectedBackendConfig> {
    let runtime_ids = current.selected_backend_runtime_ids(selected);
    selected_backend_config_with_overrides_and_runtime_ids(
        current,
        selected,
        current.backend_protocol(&selected.backend_name),
        current.backend_policy(&selected.backend_name),
        runtime_ids,
    )
}

pub(crate) fn selected_backend_config_cached(
    cache: &SelectedBackendConfigCache,
    current: &Snapshot,
    selected: &SelectedBackend,
) -> pingora::Result<Arc<SelectedBackendConfig>> {
    let runtime_ids = current.selected_backend_runtime_ids(selected);
    let Some(key) = selected_backend_config_cache_key(selected.route_kind, runtime_ids) else {
        return selected_backend_config_with_overrides_and_runtime_ids(
            current,
            selected,
            current.backend_protocol(&selected.backend_name),
            current.backend_policy(&selected.backend_name),
            runtime_ids,
        )
        .map(Arc::new);
    };

    if let Some(config) = cache.get(current.id.as_str(), key) {
        return Ok(config);
    }

    let config = Arc::new(selected_backend_config_with_overrides_and_runtime_ids(
        current,
        selected,
        current.backend_protocol(&selected.backend_name),
        current.backend_policy(&selected.backend_name),
        runtime_ids,
    )?);
    Ok(cache.insert_or_get(current.id.as_str(), key, config))
}

pub(crate) fn selected_backend_config_cached_for_fast_path(
    cache: &SelectedBackendConfigCache,
    current: &Snapshot,
    selected: &ntgw_ir::CompiledSelectedHttpBackend,
) -> pingora::Result<Arc<SelectedBackendConfig>> {
    let key = selected_backend_config_cache_key(selected.route_kind, selected.runtime_ids);
    if let Some(key) = key
        && let Some(config) = cache.get(current.id.as_str(), key)
    {
        return Ok(config);
    }

    let selected_backend = SelectedBackend {
        route_kind: selected.route_kind,
        route_name: selected.route_name.clone(),
        route_namespace: selected.route_namespace.clone(),
        rule_index: selected.rule_index,
        route_annotations: BTreeMap::new(),
        listener_name: selected.listener_name.clone(),
        listener_protocol: selected.listener_protocol.clone(),
        backend: selected.backend.clone(),
        backend_name: selected.backend_name.clone(),
        filters: Vec::new(),
        matched_http_path: Some(selected.matched_http_path.clone()),
        timeouts: None,
        retry: None,
        session_persistence: None,
        backend_tls: None,
        route_policy: None,
    };
    let config = Arc::new(selected_backend_config_with_overrides_and_runtime_ids(
        current,
        &selected_backend,
        current.backend_protocol(&selected.backend_name),
        current.backend_policy(&selected.backend_name),
        selected.runtime_ids,
    )?);

    Ok(match key {
        Some(key) => cache.insert_or_get(current.id.as_str(), key, config),
        None => config,
    })
}

impl SelectedBackendConfigCache {
    fn get(
        &self,
        snapshot_id: &str,
        key: SelectedBackendConfigCacheKey,
    ) -> Option<Arc<SelectedBackendConfig>> {
        if snapshot_id.is_empty() {
            return None;
        }

        SELECTED_BACKEND_CONFIG_CACHE.with(|cache| {
            let state = cache.borrow();
            if state.snapshot_id != snapshot_id {
                return None;
            }
            state.entries.get(&key).cloned()
        })
    }

    fn insert_or_get(
        &self,
        snapshot_id: &str,
        key: SelectedBackendConfigCacheKey,
        config: Arc<SelectedBackendConfig>,
    ) -> Arc<SelectedBackendConfig> {
        if snapshot_id.is_empty() {
            return config;
        }

        SELECTED_BACKEND_CONFIG_CACHE.with(|cache| {
            let mut state = cache.borrow_mut();
            if state.snapshot_id != snapshot_id {
                state.snapshot_id.clear();
                state.snapshot_id.push_str(snapshot_id);
                state.entries.clear();
            }
            if let Some(existing) = state.entries.get(&key) {
                return Arc::clone(existing);
            }
            state.entries.insert(key, Arc::clone(&config));
            config
        })
    }
}

fn selected_backend_config_cache_key(
    route_kind: RouteKind,
    runtime_ids: SelectedBackendRuntimeIds,
) -> Option<SelectedBackendConfigCacheKey> {
    Some(SelectedBackendConfigCacheKey {
        route_kind: route_kind_cache_key(route_kind),
        listener: runtime_ids.listener?.as_u64(),
        route: runtime_ids.route?.as_u64(),
        rule: runtime_ids.rule.map(|id| id.as_u64()).unwrap_or_default(),
        backend: runtime_ids.backend?.as_u64(),
        endpoint: runtime_ids.endpoint?.as_u64(),
    })
}

fn route_kind_cache_key(route_kind: RouteKind) -> u8 {
    match route_kind {
        RouteKind::Http => 1,
        RouteKind::Grpc => 2,
        RouteKind::Tcp => 3,
        RouteKind::Udp => 4,
        RouteKind::Tls => 5,
    }
}

fn selected_backend_config_with_overrides_and_runtime_ids(
    current: &Snapshot,
    selected: &SelectedBackend,
    protocol: Option<&str>,
    policy: Option<&BackendPolicy>,
    runtime_ids: SelectedBackendRuntimeIds,
) -> pingora::Result<SelectedBackendConfig> {
    selected_backend_config_from_parts(current, selected, protocol, policy, runtime_ids)
}

#[cfg(test)]
pub(crate) fn selected_backend_config_with_overrides(
    current: &Snapshot,
    selected: &SelectedBackend,
    protocol: Option<&str>,
    policy: Option<&BackendPolicy>,
) -> pingora::Result<SelectedBackendConfig> {
    selected_backend_config_with_overrides_and_runtime_ids(
        current,
        selected,
        protocol,
        policy,
        current.selected_backend_runtime_ids(selected),
    )
}

fn selected_backend_config_from_parts(
    current: &Snapshot,
    selected: &SelectedBackend,
    protocol: Option<&str>,
    policy: Option<&BackendPolicy>,
    runtime_ids: SelectedBackendRuntimeIds,
) -> pingora::Result<SelectedBackendConfig> {
    let tls_validation = policy.and_then(|item| item.tls_validation.as_ref());
    let tls_enabled = is_tls_backend_protocol(protocol)
        || tls_validation.is_some()
        || selected.backend_tls.is_some();
    let sni = if tls_enabled {
        backend_tls_sni_name(selected, tls_validation).unwrap_or_default()
    } else {
        String::new()
    };
    let use_http2 = matches!(selected.route_kind, ntgw_ir::RouteKind::Grpc)
        || is_http2_backend_protocol(protocol);
    let connect_timeout = selected
        .route_policy
        .as_ref()
        .and_then(|rp| rp.timeout.as_ref())
        .and_then(|t| t.connect)
        .map(std::time::Duration::from_millis)
        .or_else(|| policy.and_then(|item| item.connect_timeout));
    let request_timeout = effective_request_timeout_with_route_policy(
        &selected.route_policy,
        policy,
        selected.timeouts.as_ref(),
    );
    let backend_tls_validation = resolve_backend_tls_validation(tls_validation)?;
    let client_cert_key = resolve_backend_client_cert_key(current, selected.backend_tls.as_ref())?;

    Ok(SelectedBackendConfig {
        runtime: current.endpoint_runtime_handle(selected),
        runtime_ids,
        peer_address: UpstreamPeerAddress::from_backend_address(&selected.backend.address),
        peer_port: selected.backend.port as u16,
        tls_enabled,
        sni,
        use_http2,
        connect_timeout,
        request_timeout,
        backend_tls_validation,
        client_cert_key,
        traffic_topology: TrafficTopology::from_parts(
            selected.listener_name.as_str(),
            route_kind_name(&selected.route_kind),
            selected.route_namespace.as_str(),
            selected.route_name.as_str(),
            selected.backend_name.as_str(),
        ),
    })
}
