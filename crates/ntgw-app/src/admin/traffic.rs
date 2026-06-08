use ntgw_ir::{RuntimeId, Snapshot};
use ntgw_observability::TrafficSnapshot;
use serde_json::Value;

use super::{filters::runtime_ref_value, types::ApiError};

pub(super) fn traffic_view_value(
    snapshot: &Snapshot,
    traffic: TrafficSnapshot,
) -> Result<Value, ApiError> {
    let mut value = serde_json::to_value(traffic).map_err(|err| {
        ApiError::internal(format!("traffic snapshot serialization failed: {err}"))
    })?;
    insert_node_runtime_refs(snapshot, &mut value)?;
    Ok(value)
}

fn insert_node_runtime_refs(snapshot: &Snapshot, value: &mut Value) -> Result<(), ApiError> {
    let nodes = value
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| ApiError::internal("traffic nodes did not serialize as an array"))?;

    for node in nodes {
        let Some(runtime_id) = node
            .get("runtimeId")
            .and_then(Value::as_str)
            .and_then(RuntimeId::parse_hex)
        else {
            continue;
        };
        let Some(resource_ref) = snapshot.runtime_resource_ref(runtime_id) else {
            continue;
        };
        let Some(object) = node.as_object_mut() else {
            continue;
        };
        object.insert("runtimeRef".to_string(), runtime_ref_value(resource_ref));
    }

    Ok(())
}
