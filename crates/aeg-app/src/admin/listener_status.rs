use aeg_ir::Snapshot;
use aeg_observability::RuntimeStatsSnapshot;

use super::{
    build_listener_runtime_status, filter_listeners,
    types::{ApiError, ListenerListQuery, ListenerRuntimeStatus},
};

#[derive(Default)]
struct ListenerStatusFilters {
    runtime_plane: Option<String>,
    current_status: Option<String>,
    current_failure: Option<bool>,
    serving_snapshot: Option<String>,
    serving_version: Option<String>,
    serving_state: Option<String>,
    recovery_state: Option<String>,
    has_ever_failed: Option<bool>,
    attention_required: Option<bool>,
    attention_reason: Option<String>,
    recovered_from_failure: Option<bool>,
    attempt_progress: Option<String>,
    unrecovered_failure_age: Option<String>,
}

pub(super) fn collect_listener_runtime_statuses(
    snapshot: &Snapshot,
    runtime: &RuntimeStatsSnapshot,
    query: &ListenerListQuery,
) -> Result<Vec<ListenerRuntimeStatus>, ApiError> {
    let listeners = filter_listeners(snapshot, query)?;
    let filters = parse_listener_status_filters(query)?;

    Ok(listeners
        .iter()
        .map(|listener| build_listener_runtime_status(listener, snapshot, runtime))
        .filter(|status| matches_listener_status_filters(status, &filters))
        .collect())
}

fn parse_listener_status_filters(
    query: &ListenerListQuery,
) -> Result<ListenerStatusFilters, ApiError> {
    Ok(ListenerStatusFilters {
        runtime_plane: parse_listener_runtime_plane_filter(query.runtime_plane.as_deref())?,
        current_status: parse_listener_current_status_filter(query.current_status.as_deref())?,
        current_failure: query.current_failure,
        serving_snapshot: parse_listener_serving_snapshot_filter(
            query.serving_snapshot.as_deref(),
        )?,
        serving_version: query
            .serving_version
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        serving_state: parse_listener_serving_state_filter(query.serving_state.as_deref())?,
        recovery_state: parse_listener_recovery_state_filter(query.recovery_state.as_deref())?,
        has_ever_failed: query.has_ever_failed,
        attention_required: query.attention_required,
        attention_reason: parse_listener_attention_reason_filter(
            query.attention_reason.as_deref(),
        )?,
        recovered_from_failure: query.recovered_from_failure,
        attempt_progress: parse_listener_attempt_progress_filter(
            query.attempt_progress.as_deref(),
        )?,
        unrecovered_failure_age: parse_listener_unrecovered_failure_age_filter(
            query.unrecovered_failure_age.as_deref(),
        )?,
    })
}

fn matches_listener_status_filters(
    status: &ListenerRuntimeStatus,
    filters: &ListenerStatusFilters,
) -> bool {
    filters
        .runtime_plane
        .as_deref()
        .map(|value| status.runtime_plane == value)
        .unwrap_or(true)
        && filters
            .current_status
            .as_deref()
            .map(|value| status.listener_current_status == value)
            .unwrap_or(true)
        && filters
            .current_failure
            .map(|value| status.listener_current_failure == value)
            .unwrap_or(true)
        && match filters.serving_snapshot.as_deref() {
            Some("current") => status.listener_serving_current_snapshot,
            Some("last-good") => status.listener_serving_last_good_snapshot,
            None => true,
            Some(_) => false,
        }
        && filters
            .serving_version
            .as_deref()
            .map(|value| status.listener_serving_version == value)
            .unwrap_or(true)
        && filters
            .serving_state
            .as_deref()
            .map(|value| status.listener_serving_state == value)
            .unwrap_or(true)
        && filters
            .recovery_state
            .as_deref()
            .map(|value| status.listener_recovery_state == value)
            .unwrap_or(true)
        && filters
            .has_ever_failed
            .map(|value| status.listener_has_ever_failed == value)
            .unwrap_or(true)
        && filters
            .attention_required
            .map(|value| status.listener_attention_required == value)
            .unwrap_or(true)
        && filters
            .attention_reason
            .as_deref()
            .map(|value| {
                status
                    .listener_attention_reasons
                    .iter()
                    .any(|reason| reason == value)
            })
            .unwrap_or(true)
        && filters
            .recovered_from_failure
            .map(|value| status.listener_recovered_from_failure == value)
            .unwrap_or(true)
        && match filters.attempt_progress.as_deref() {
            Some("awaiting-current") => status.listener_awaiting_current_attempt,
            Some("blocked-current") => status.listener_current_attempt_blocked,
            Some("other") => {
                !status.listener_awaiting_current_attempt
                    && !status.listener_current_attempt_blocked
            }
            None => true,
            Some(_) => false,
        }
        && match filters.unrecovered_failure_age.as_deref() {
            Some("current") => status.listener_unrecovered_current_snapshot_failure,
            Some("historical") => status.listener_unrecovered_historical_failure,
            Some("none") => {
                !status.listener_unrecovered_current_snapshot_failure
                    && !status.listener_unrecovered_historical_failure
            }
            None => true,
            Some(_) => false,
        }
}

fn parse_listener_runtime_plane_filter(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    let value = raw.map(str::trim).unwrap_or_default();
    if value.is_empty() {
        return Ok(None);
    }

    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "http" | "tls" | "stream" | "none" => Ok(Some(value)),
        _ => Err(ApiError::bad_request("invalid listener runtime plane")),
    }
}

fn parse_listener_current_status_filter(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    let value = raw.map(str::trim).unwrap_or_default();
    if value.is_empty() {
        return Ok(None);
    }

    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "idle" | "warming" | "pending" | "accepted" | "retained" | "rejected" | "stale" => {
            Ok(Some(value))
        }
        _ => Err(ApiError::bad_request("invalid listener current status")),
    }
}

fn parse_listener_serving_snapshot_filter(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    let value = raw.map(str::trim).unwrap_or_default();
    if value.is_empty() {
        return Ok(None);
    }

    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "current" | "last-good" => Ok(Some(value)),
        _ => Err(ApiError::bad_request("invalid listener serving snapshot")),
    }
}

fn parse_listener_serving_state_filter(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    let value = raw.map(str::trim).unwrap_or_default();
    if value.is_empty() {
        return Ok(None);
    }

    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "none" | "current-accepted" | "current-retained" | "last-good-rejected"
        | "last-good-stale" => Ok(Some(value)),
        _ => Err(ApiError::bad_request("invalid listener serving state")),
    }
}

fn parse_listener_attention_reason_filter(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    let value = raw.map(str::trim).unwrap_or_default();
    if value.is_empty() {
        return Ok(None);
    }

    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "pending" | "rejected" | "stale" | "unrecovered_failure" => Ok(Some(value)),
        _ => Err(ApiError::bad_request("invalid listener attention reason")),
    }
}

fn parse_listener_recovery_state_filter(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    let value = raw.map(str::trim).unwrap_or_default();
    if value.is_empty() {
        return Ok(None);
    }

    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "idle"
        | "warming"
        | "steady"
        | "recovered"
        | "awaiting-current"
        | "blocked-current"
        | "unrecovered-current"
        | "unrecovered-historical"
        | "drifted-last-good" => Ok(Some(value)),
        _ => Err(ApiError::bad_request("invalid listener recovery state")),
    }
}

fn parse_listener_attempt_progress_filter(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    let value = raw.map(str::trim).unwrap_or_default();
    if value.is_empty() {
        return Ok(None);
    }

    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "awaiting-current" | "blocked-current" | "other" => Ok(Some(value)),
        _ => Err(ApiError::bad_request("invalid listener attempt progress")),
    }
}

fn parse_listener_unrecovered_failure_age_filter(
    raw: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let value = raw.map(str::trim).unwrap_or_default();
    if value.is_empty() {
        return Ok(None);
    }

    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "current" | "historical" | "none" => Ok(Some(value)),
        _ => Err(ApiError::bad_request(
            "invalid listener unrecovered failure age",
        )),
    }
}
