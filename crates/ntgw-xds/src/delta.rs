use std::collections::HashMap;

use anyhow::{Context, Result};
use prost::Message;
use tokio::sync::{mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use tracing::{debug, info, warn};

use ntgw_proto::gateway::control::v1::{
    ConfigSnapshot, DeltaDiscoveryRequest, DeltaDiscoveryResponse, DiscoveryResultStatus,
    delta_discovery_service_client::DeltaDiscoveryServiceClient,
};

use crate::features::supported_features;

const SUBSCRIPTIONS: &[&str] = &[
    "type.googleapis.com/gateway.control.v1.Listener",
    "type.googleapis.com/gateway.control.v1.HttpRoute",
    "type.googleapis.com/gateway.control.v1.GrpcRoute",
    "type.googleapis.com/gateway.control.v1.StreamRoute",
    "type.googleapis.com/gateway.control.v1.BackendCluster",
    "type.googleapis.com/gateway.control.v1.SecretMaterial",
];
const SUBSCRIPTION_COUNT: usize = SUBSCRIPTIONS.len();

pub async fn delta_connect_loop(
    node_id: String,
    cluster: String,
    mut shutdown: watch::Receiver<bool>,
    mut client: DeltaDiscoveryServiceClient<Channel>,
    on_snapshot: impl Fn(ConfigSnapshot),
) -> Result<()> {
    let (tx, rx) = mpsc::channel(8);
    let supported = supported_features();
    let subs: Vec<String> = SUBSCRIPTIONS.iter().map(|s| s.to_string()).collect();

    tx.send(DeltaDiscoveryRequest {
        node_id: node_id.clone(),
        cluster: cluster.clone(),
        resource_names_subscribe: subs.clone(),
        supported_features: supported.clone(),
        ..Default::default()
    })
    .await
    .context("send initial delta request")?;

    let response = client
        .delta_stream_configuration(ReceiverStream::new(rx))
        .await
        .context("open delta stream")?;

    let mut stream = response.into_inner();
    info!(
        "delta xDS stream established, subscribing to {} types",
        SUBSCRIPTIONS.len()
    );

    let mut cached = ConfigSnapshot::default();
    let mut seen_types: HashMap<String, bool> = HashMap::with_capacity(SUBSCRIPTION_COUNT);
    let mut version = String::new();

    loop {
        let msg = tokio::select! {
            biased;
            _ = shutdown.changed() => return Ok(()),
            result = stream.message() => match result {
                Ok(Some(m)) => m,
                Ok(None) => return Ok(()),
                Err(e) => return Err(e.into()),
            },
        };

        if seen_types.len() < SUBSCRIPTION_COUNT {
            seen_types.entry(msg.type_url.clone()).or_insert(true);
        }

        let initialized = seen_types.len() >= SUBSCRIPTION_COUNT;
        let version_changed = msg.system_version_info != version;

        if msg.non_incremental {
            clear_type(&mut cached, &msg.type_url);
        }

        for name in &msg.removed_resources {
            remove_resource(&mut cached, &msg.type_url, name);
        }

        for resource in &msg.resources {
            if resource.name.is_empty() {
                continue;
            }
            if let Err(e) = upsert_resource(&mut cached, &msg.type_url, &resource.name, resource) {
                warn!(type_url=%msg.type_url, name=%resource.name, error=%e, "delta decode failed");
            }
        }

        if initialized && version_changed {
            version = msg.system_version_info.clone();
            debug!(version=%version, listeners=%cached.listeners.len(), routes=(cached.http_routes.len() + cached.grpc_routes.len()), "delta snapshot applied");
            on_snapshot(cached.clone());

            tx.send(DeltaDiscoveryRequest {
                node_id: node_id.clone(),
                cluster: cluster.clone(),
                response_nonce: msg.nonce,
                type_url: msg.type_url.clone(),
                result_status: DiscoveryResultStatus::Ack as i32,
                supported_features: supported.clone(),
                ..Default::default()
            })
            .await
            .context("send delta ack")?;
        }
    }
}

fn clear_type(snap: &mut ConfigSnapshot, type_url: &str) {
    if type_url.contains("Listener") {
        snap.listeners.clear();
    } else if type_url.contains("HttpRoute") {
        snap.http_routes.clear();
    } else if type_url.contains("GrpcRoute") {
        snap.grpc_routes.clear();
    } else if type_url.contains("StreamRoute") {
        snap.stream_routes.clear();
    } else if type_url.contains("BackendCluster") {
        snap.backends.clear();
    } else if type_url.contains("SecretMaterial") {
        snap.secrets.clear();
    }
}

fn remove_resource(snap: &mut ConfigSnapshot, type_url: &str, name: &str) {
    if type_url.contains("Listener") {
        snap.listeners.retain(|l| l.name != name);
    } else if type_url.contains("HttpRoute") {
        snap.http_routes.retain(|r| r.name != name);
    } else if type_url.contains("GrpcRoute") {
        snap.grpc_routes.retain(|r| r.name != name);
    } else if type_url.contains("StreamRoute") {
        snap.stream_routes.retain(|r| r.name != name);
    } else if type_url.contains("BackendCluster") {
        snap.backends.retain(|b| b.name != name);
    } else if type_url.contains("SecretMaterial") {
        snap.secrets.retain(|s| s.name != name);
    }
}

fn upsert_resource(
    snap: &mut ConfigSnapshot,
    type_url: &str,
    name: &str,
    resource: &ntgw_proto::gateway::control::v1::Resource,
) -> Result<()> {
    use ntgw_proto::gateway::control::v1::{
        BackendCluster, GrpcRoute, HttpRoute, Listener, SecretMaterial, StreamRoute,
    };

    let Some(any) = resource.resource.as_ref() else {
        return Ok(());
    };
    let bytes: &[u8] = &any.value;

    if type_url.contains("Listener") {
        snap.listeners.retain(|l| l.name != name);
        snap.listeners
            .push(Listener::decode(bytes).context("decode Listener")?);
    } else if type_url.contains("HttpRoute") {
        snap.http_routes.retain(|rt| rt.name != name);
        snap.http_routes
            .push(HttpRoute::decode(bytes).context("decode HttpRoute")?);
    } else if type_url.contains("GrpcRoute") {
        snap.grpc_routes.retain(|rt| rt.name != name);
        snap.grpc_routes
            .push(GrpcRoute::decode(bytes).context("decode GrpcRoute")?);
    } else if type_url.contains("StreamRoute") {
        snap.stream_routes.retain(|rt| rt.name != name);
        snap.stream_routes
            .push(StreamRoute::decode(bytes).context("decode StreamRoute")?);
    } else if type_url.contains("BackendCluster") {
        snap.backends.retain(|b| b.name != name);
        snap.backends
            .push(BackendCluster::decode(bytes).context("decode BackendCluster")?);
    } else if type_url.contains("SecretMaterial") {
        snap.secrets.retain(|s| s.name != name);
        snap.secrets
            .push(SecretMaterial::decode(bytes).context("decode SecretMaterial")?);
    } else {
        warn!("unknown delta type_url: {}", type_url);
    }
    Ok(())
}
