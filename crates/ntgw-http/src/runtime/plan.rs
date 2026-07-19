use std::collections::{BTreeMap, BTreeSet};
use std::io;

use tracing::{debug, info, warn};

use ntgw_ir::Snapshot;
use ntgw_observability::RuntimeStatsSnapshot;

use super::listener_plan::{
    bind_variants, desired_listener_protocol, is_http3_protocol, is_l7_protocol,
    listener_bind_addrs,
};
use super::options::RuntimeOptions;
use super::{
    should_defer_http_listener_bind_handoff, should_suppress_unavailable_bind_warning,
    tls_passthrough_binds,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListenerPlan {
    pub(crate) listeners: Vec<PlannedListener>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedListener {
    pub(crate) name: String,
    pub(crate) bind: String,
    pub(crate) protocol: ListenerProtocol,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ListenerProtocol {
    Plain,
    Tls(TlsMaterial),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TlsMaterial {
    pub(crate) identities: Vec<TlsIdentity>,
    pub(crate) min_version: String,
    pub(crate) max_version: String,
    pub(crate) client_ca_bundle_pem: Option<String>,
    pub(crate) frontend_validation_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TlsIdentity {
    pub(crate) secret_ref: String,
    pub(crate) cert_pem: String,
    pub(crate) key_pem: String,
    pub(crate) match_names: Vec<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ListenerPlanBuildResult {
    pub(crate) plan: Option<ListenerPlan>,
    pub(crate) retry_start: bool,
    pub(crate) deferred_binds: Vec<String>,
}

#[cfg(test)]
pub(crate) fn build_listener_plan(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
    active_plan: Option<&ListenerPlan>,
) -> Option<ListenerPlan> {
    let active_binds = active_listener_binds(active_plan);
    build_listener_plan_with_bind_checker(snapshot, runtime, &active_binds, |_| Ok(()))
}

pub(crate) fn build_listener_plan_for_runtime(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
    active_binds: &BTreeSet<String>,
    runtime_state: &RuntimeStatsSnapshot,
) -> ListenerPlanBuildResult {
    #[cfg(test)]
    {
        build_listener_plan_with_bind_checker_for_runtime(
            snapshot,
            runtime,
            active_binds,
            |_| Ok(()),
            runtime_state,
        )
    }

    #[cfg(not(test))]
    {
        build_listener_plan_with_bind_checker_for_runtime(
            snapshot,
            runtime,
            active_binds,
            super::probe_listener_bind,
            runtime_state,
        )
    }
}

pub(crate) fn active_listener_binds_for_plan_build(
    active_plan: Option<&ListenerPlan>,
    _force_reload: bool,
) -> BTreeSet<String> {
    active_listener_binds(active_plan)
}

#[cfg(test)]
pub(crate) fn build_listener_plan_with_bind_checker<F>(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
    active_binds: &BTreeSet<String>,
    mut bind_checker: F,
) -> Option<ListenerPlan>
where
    F: FnMut(&str) -> io::Result<()>,
{
    build_listener_plan_with_bind_checker_inner(
        snapshot,
        runtime,
        active_binds,
        &mut bind_checker,
        &RuntimeStatsSnapshot::default(),
        true,
    )
    .plan
}

pub(crate) fn build_listener_plan_with_bind_checker_for_runtime<F>(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
    active_binds: &BTreeSet<String>,
    mut bind_checker: F,
    runtime_state: &RuntimeStatsSnapshot,
) -> ListenerPlanBuildResult
where
    F: FnMut(&str) -> io::Result<()>,
{
    build_listener_plan_with_bind_checker_inner(
        snapshot,
        runtime,
        active_binds,
        &mut bind_checker,
        runtime_state,
        false,
    )
}

pub(crate) fn build_listener_plan_with_bind_checker_inner<F>(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
    active_binds: &BTreeSet<String>,
    mut bind_checker: F,
    runtime_state: &RuntimeStatsSnapshot,
    include_tls_terminating_listeners: bool,
) -> ListenerPlanBuildResult
where
    F: FnMut(&str) -> io::Result<()>,
{
    let has_declared_l7 = snapshot
        .listeners
        .iter()
        .any(|listener| is_l7_protocol(&listener.protocol));
    let blocked_tls_binds = tls_passthrough_binds(snapshot, runtime);
    let mut endpoints: BTreeMap<String, PlannedListener> = BTreeMap::new();
    let mut result = ListenerPlanBuildResult::default();

    for listener in &snapshot.listeners {
        match desired_listener_protocol(listener, snapshot, runtime) {
            Some(protocol) => {
                if matches!(protocol, ListenerProtocol::Tls(_))
                    && !include_tls_terminating_listeners
                {
                    continue;
                }
                for bind in listener_bind_addrs(listener, runtime) {
                    if !active_binds.contains(&bind)
                        && let Err(err) = bind_checker(bind.as_str())
                    {
                        if should_defer_http_listener_bind_handoff(
                            snapshot,
                            runtime_state,
                            &protocol,
                            bind.as_str(),
                            &blocked_tls_binds,
                            &err,
                        ) {
                            if !result.deferred_binds.contains(&bind) {
                                result.deferred_binds.push(bind.clone());
                            }
                            result.retry_start = true;
                            info!(
                                listener = %listener.name,
                                bind = %bind,
                                version = %snapshot.id,
                                stream_last_good_version = %runtime_state.stream_last_good_reload_version,
                                "delaying tls-terminating http listener until stream runtime releases the shared bind"
                            );
                            continue;
                        }
                        if should_suppress_unavailable_bind_warning(bind.as_str(), &err) {
                            debug!(
                                listener = %listener.name,
                                bind = %bind,
                                error = %err,
                                "skipping http listener because IPv6 address family is unavailable"
                            );
                            continue;
                        }
                        warn!(
                            listener = %listener.name,
                            bind = %bind,
                            error = %err,
                            "skipping http listener because the bind address is unavailable"
                        );
                        continue;
                    }

                    if matches!(protocol, ListenerProtocol::Tls(_))
                        && blocked_tls_binds.contains(&bind)
                    {
                        warn!(
                            listener = %listener.name,
                            bind = %bind,
                            "skipping tls-terminating http listener because a tls passthrough listener claims the same bind address"
                        );
                        continue;
                    }

                    let planned = PlannedListener {
                        name: listener.name.clone(),
                        bind: bind.clone(),
                        protocol: protocol.clone(),
                    };

                    match endpoints.get(&bind) {
                        Some(existing) if existing.protocol != planned.protocol => {
                            warn!(
                                bind = %bind,
                                first_listener = %existing.name,
                                skipped_listener = %planned.name,
                                "conflicting http listener configuration on the same bind address; keeping the first listener"
                            );
                        }
                        Some(_) => {}
                        None => {
                            endpoints.insert(bind, planned);
                        }
                    }
                }
            }
            None if is_http3_protocol(&listener.protocol) => {
                warn!(
                    listener = %listener.name,
                    "HTTP/3 listener requested but unavailable in the current Nantian build"
                );
            }
            None => {}
        }
    }

    if endpoints.is_empty() && !has_declared_l7 && !runtime.default_listen_addr.is_empty() {
        for default_bind in bind_variants(runtime.default_listen_addr.as_str(), runtime.enable_ipv6)
        {
            if !active_binds.contains(&default_bind)
                && let Err(err) = bind_checker(default_bind.as_str())
            {
                if should_suppress_unavailable_bind_warning(default_bind.as_str(), &err) {
                    debug!(
                        bind = %default_bind,
                        error = %err,
                        "skipping default http listener because IPv6 address family is unavailable"
                    );
                    continue;
                }
                warn!(
                    bind = %default_bind,
                    error = %err,
                    "skipping default http listener because the bind address is unavailable"
                );
                continue;
            }

            endpoints.insert(
                default_bind.clone(),
                PlannedListener {
                    name: "runtime/default-http".to_string(),
                    bind: default_bind,
                    protocol: ListenerProtocol::Plain,
                },
            );
        }
    }

    result.plan = (!endpoints.is_empty()).then(|| ListenerPlan {
        listeners: endpoints.into_values().collect(),
    });
    result
}

fn active_listener_binds(active_plan: Option<&ListenerPlan>) -> BTreeSet<String> {
    active_plan
        .map(|plan| {
            plan.listeners
                .iter()
                .map(|listener| listener.bind.clone())
                .collect()
        })
        .unwrap_or_default()
}
