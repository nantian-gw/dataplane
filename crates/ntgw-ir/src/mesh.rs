use serde::{Deserialize, Serialize};

use crate::Listener;

pub const FRONTEND_KIND_METADATA_KEY: &str = "nantian.dev/frontend-kind";
pub const FRONTEND_NAMESPACE_METADATA_KEY: &str = "nantian.dev/frontend-namespace";
pub const FRONTEND_NAME_METADATA_KEY: &str = "nantian.dev/frontend-name";
pub const FRONTEND_PORT_METADATA_KEY: &str = "nantian.dev/frontend-port";
pub const FRONTEND_KIND_SERVICE: &str = "Service";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Workload {
    pub namespace: String,
    pub name: String,
    pub ip: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParentRef {
    pub group: String,
    pub kind: String,
    pub namespace: String,
    pub name: String,
    pub section_name: String,
    pub port: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceFrontendRef {
    pub namespace: String,
    pub name: String,
    pub port: u32,
}

#[must_use]
pub fn service_frontend(listener: &Listener) -> Option<ServiceFrontendRef> {
    if listener
        .metadata
        .get(FRONTEND_KIND_METADATA_KEY)
        .is_none_or(|value| value != FRONTEND_KIND_SERVICE)
    {
        return None;
    }

    let namespace = listener
        .metadata
        .get(FRONTEND_NAMESPACE_METADATA_KEY)?
        .clone();
    let name = listener.metadata.get(FRONTEND_NAME_METADATA_KEY)?.clone();
    let port = listener
        .metadata
        .get(FRONTEND_PORT_METADATA_KEY)?
        .parse::<u32>()
        .ok()?;

    Some(ServiceFrontendRef {
        namespace,
        name,
        port,
    })
}

pub fn is_service_parent(parent_ref: &ParentRef) -> bool {
    parent_ref.group.is_empty() && parent_ref.kind == FRONTEND_KIND_SERVICE
}

/// Checks whether a mesh route should be accepted for a service frontend listener.
///
/// Accepts the route if:
/// 1. The listener is not a service frontend, OR
/// 2. Any parent ref matches the frontend (by namespace, name, port)
///
/// The `source_namespace` parameter was removed because the workload IP
/// lookup (`workload_namespace_index`) is unreliable — it fails intermittently
/// when pods restart or IPs change between snapshot rebuilds, causing mesh
/// route instability (the route "flashes" — works for first requests, then
/// drops with InvalidBackendRefs).
///
/// The parent ref namespace check is sufficient for cross-namespace access
/// control in mesh routing.
pub fn route_accepts_service_frontend(
    snapshot: &crate::Snapshot,
    parent_refs: &[ParentRef],
    route_namespace: &str,
    listener: &Listener,
) -> bool {
    let frontend = snapshot.service_frontend_for_listener(listener);
    let Some(frontend) = frontend else {
        return true;
    };

    for parent_ref in parent_refs {
        if !is_service_parent(parent_ref) {
            continue;
        }

        let parent_namespace = if parent_ref.namespace.is_empty() {
            route_namespace
        } else {
            parent_ref.namespace.as_str()
        };
        if parent_namespace != frontend.namespace
            || parent_ref.name != frontend.name
            || (parent_ref.port != 0 && parent_ref.port != frontend.port)
        {
            continue;
        }

        // A parent ref matches the frontend. Accept the route regardless of
        // namespace. The source_namespace check was removed because workload
        // IP lookup is unreliable and causes mesh route instability.
        return true;
    }

    // No parent ref matched the frontend; accept the route (not a mesh route).
    true
}