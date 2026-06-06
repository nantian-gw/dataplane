use ntgw_proto::gateway::control::v1 as proto;
use serde::{Deserialize, Serialize};

use crate::{RequestMeta, Snapshot};

mod helpers;
mod proto_snapshot;
mod route_selection;
mod snapshot_switch;

pub use proto_snapshot::build_proto_snapshot_fixture;
pub use route_selection::build_route_selection_fixture;
pub use snapshot_switch::build_snapshot_switch_fixture;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SnapshotBenchConfig {
    pub listeners: usize,
    pub routes_per_listener: usize,
    pub backends_per_route: usize,
    pub endpoints_per_backend: usize,
}

impl Default for SnapshotBenchConfig {
    fn default() -> Self {
        Self {
            listeners: 24,
            routes_per_listener: 16,
            backends_per_route: 4,
            endpoints_per_backend: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotSwitchFixture {
    pub current: Snapshot,
    pub next: Snapshot,
    pub probe_request: RequestMeta,
    pub expected_backend_name: String,
}

#[derive(Debug, Clone)]
pub struct RouteSelectionFixture {
    pub snapshot: Snapshot,
    pub http_request: RequestMeta,
    pub grpc_request: RequestMeta,
    pub stream_listener_name: String,
    pub stream_server_name: String,
    pub expected_http_backend_name: String,
    pub expected_grpc_backend_name: String,
    pub expected_stream_backend_name: String,
}

#[derive(Debug, Clone)]
pub struct ProtoSnapshotFixture {
    pub snapshot: proto::ConfigSnapshot,
    pub expected_listener_count: usize,
    pub expected_http_routes: usize,
    pub expected_grpc_routes: usize,
    pub expected_stream_routes: usize,
    pub expected_backends: usize,
}
