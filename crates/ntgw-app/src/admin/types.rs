use ntgw_ir::{GrpcRoute, HttpRoute, Listener, StreamRoute};
use axum::{http::StatusCode, response::IntoResponse};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub(super) struct RouteListResponse {
    pub(super) http: Vec<HttpRoute>,
    pub(super) grpc: Vec<GrpcRoute>,
    pub(super) stream: Vec<StreamRoute>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub(super) struct RouteListValueResponse {
    pub(super) http: Vec<Value>,
    pub(super) grpc: Vec<Value>,
    pub(super) stream: Vec<Value>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(super) struct ListenerRuntimeStatus {
    #[serde(flatten)]
    pub(super) listener: Listener,
    #[serde(rename = "runtimeId", skip_serializing_if = "Option::is_none")]
    pub(super) runtime_id: Option<String>,
    pub(super) runtime_plane: String,
    pub(super) runtime_required: bool,
    pub(super) runtime_current_status: String,
    pub(super) runtime_current_accepted: bool,
    pub(super) runtime_current_rejected: bool,
    pub(super) listener_current_status: String,
    pub(super) listener_current_accepted: bool,
    pub(super) listener_current_retained: bool,
    pub(super) listener_current_rejected: bool,
    pub(super) listener_current_stale: bool,
    pub(super) listener_serving_state: String,
    pub(super) listener_recovery_state: String,
    pub(super) listener_attention_required: bool,
    pub(super) listener_attention_reasons: Vec<String>,
    pub(super) listener_current_failure: bool,
    pub(super) listener_awaiting_current_attempt: bool,
    pub(super) listener_current_attempt_blocked: bool,
    pub(super) listener_unrecovered_current_snapshot_failure: bool,
    pub(super) listener_unrecovered_historical_failure: bool,
    pub(super) listener_current_failure_version: String,
    pub(super) listener_current_failure_message: String,
    pub(super) listener_attempts: u64,
    pub(super) listener_failures: u64,
    pub(super) listener_last_attempt_version: String,
    pub(super) listener_last_good_version: String,
    pub(super) listener_last_failure_version: String,
    pub(super) listener_last_failure_message: String,
    pub(super) listener_last_apply_unix_seconds: u64,
    pub(super) listener_last_failure_unix_seconds: u64,
    pub(super) listener_has_ever_failed: bool,
    pub(super) listener_recovered_from_failure: bool,
    pub(super) listener_recovery_version: String,
    pub(super) listener_recovery_unix_seconds: u64,
    pub(super) listener_serving_version: String,
    pub(super) listener_serving_current_snapshot: bool,
    pub(super) listener_serving_last_good_snapshot: bool,
    pub(super) listener_recent_events: Vec<ntgw_observability::RuntimeListenerEvent>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ListenerListQuery {
    pub(super) name: Option<String>,
    pub(super) protocol: Option<String>,
    pub(super) hostname: Option<String>,
    #[serde(rename = "runtimeId")]
    pub(super) runtime_id: Option<String>,
    #[serde(rename = "attachedRoute")]
    pub(super) attached_route: Option<String>,
    #[serde(rename = "runtimePlane")]
    pub(super) runtime_plane: Option<String>,
    #[serde(rename = "currentStatus")]
    pub(super) current_status: Option<String>,
    #[serde(rename = "currentFailure")]
    pub(super) current_failure: Option<bool>,
    #[serde(rename = "hasEverFailed")]
    pub(super) has_ever_failed: Option<bool>,
    #[serde(rename = "attentionRequired")]
    pub(super) attention_required: Option<bool>,
    #[serde(rename = "attentionReason")]
    pub(super) attention_reason: Option<String>,
    #[serde(rename = "recoveredFromFailure")]
    pub(super) recovered_from_failure: Option<bool>,
    #[serde(rename = "attemptProgress")]
    pub(super) attempt_progress: Option<String>,
    #[serde(rename = "unrecoveredFailureAge")]
    pub(super) unrecovered_failure_age: Option<String>,
    #[serde(rename = "servingSnapshot")]
    pub(super) serving_snapshot: Option<String>,
    #[serde(rename = "servingVersion")]
    pub(super) serving_version: Option<String>,
    #[serde(rename = "servingState")]
    pub(super) serving_state: Option<String>,
    #[serde(rename = "recoveryState")]
    pub(super) recovery_state: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct RouteListQuery {
    pub(super) kind: Option<String>,
    pub(super) namespace: Option<String>,
    pub(super) name: Option<String>,
    pub(super) hostname: Option<String>,
    #[serde(rename = "runtimeId")]
    pub(super) runtime_id: Option<String>,
    #[serde(rename = "ruleRuntimeId")]
    pub(super) rule_runtime_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct BackendListQuery {
    pub(super) namespace: Option<String>,
    pub(super) name: Option<String>,
    pub(super) protocol: Option<String>,
    #[serde(rename = "runtimeId")]
    pub(super) runtime_id: Option<String>,
    #[serde(rename = "endpointRuntimeId")]
    pub(super) endpoint_runtime_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListenerPath {
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct RoutePath {
    pub(super) kind: String,
    pub(super) namespace: String,
    pub(super) name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct BackendPath {
    pub(super) namespace: String,
    pub(super) name: String,
}

#[derive(Debug)]
pub(super) struct ApiError {
    pub(super) status: StatusCode,
    pub(super) message: String,
}

impl ApiError {
    pub(super) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    pub(super) fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub(super) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
    }
}
