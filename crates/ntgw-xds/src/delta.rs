use std::collections::HashMap;

use anyhow::{Context, Result};
use tokio::sync::{watch, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use tracing::info;

use ntgw_proto::gateway::control::v1::{
    ConfigSnapshot,
    delta_discovery_service_client::DeltaDiscoveryServiceClient,
    DeltaDiscoveryRequest, DeltaDiscoveryResponse,
    DiscoveryResultStatus,
};

use crate::features::supported_features;

pub async fn delta_connect_loop(
    node_id: String,
    cluster: String,
    mut shutdown: watch::Receiver<bool>,
    mut client: DeltaDiscoveryServiceClient<Channel>,
    on_snapshot: impl Fn(ConfigSnapshot),
) -> Result<()> {
    let (tx, rx) = mpsc::channel(8);
    let supported = supported_features();

    tx.send(DeltaDiscoveryRequest {
        node_id: node_id.clone(),
        cluster: cluster.clone(),
        resource_names_subscribe: vec![
            "type.googleapis.com/gateway.control.v1.Listener".into(),
            "type.googleapis.com/gateway.control.v1.HttpRoute".into(),
            "type.googleapis.com/gateway.control.v1.GrpcRoute".into(),
            "type.googleapis.com/gateway.control.v1.StreamRoute".into(),
            "type.googleapis.com/gateway.control.v1.BackendCluster".into(),
            "type.googleapis.com/gateway.control.v1.SecretMaterial".into(),
        ],
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
    info!("delta xDS stream established");

    let mut cached = ConfigSnapshot::default();
    let mut seen_types: HashMap<String, bool> = HashMap::new();
    let subs_count: usize = 6;

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

        if seen_types.len() < subs_count {
            seen_types.insert(msg.type_url.clone(), true);
        }

        apply_delta_to_snapshot(&mut cached, &msg);

        if seen_types.len() >= subs_count {
            on_snapshot(cached.clone());

            tx.send(DeltaDiscoveryRequest {
                node_id: node_id.clone(),
                cluster: cluster.clone(),
                response_nonce: msg.nonce,
                type_url: msg.type_url,
                result_status: DiscoveryResultStatus::Ack as i32,
                supported_features: supported.clone(),
                ..Default::default()
            })
            .await
            .context("send delta ack")?;
        }
    }
}

fn apply_delta_to_snapshot(snap: &mut ConfigSnapshot, resp: &DeltaDiscoveryResponse) {
    if resp.non_incremental {
        if resp.type_url.contains("Listener") { snap.listeners.clear(); }
        else if resp.type_url.contains("HttpRoute") { snap.http_routes.clear(); }
        else if resp.type_url.contains("GrpcRoute") { snap.grpc_routes.clear(); }
        else if resp.type_url.contains("StreamRoute") { snap.stream_routes.clear(); }
        else if resp.type_url.contains("BackendCluster") { snap.backends.clear(); }
        else if resp.type_url.contains("SecretMaterial") { snap.secrets.clear(); }
    }

    for name in &resp.removed_resources {
        let n = name.as_str();
        if resp.type_url.contains("Listener") { snap.listeners.retain(|l| l.name != n); }
        else if resp.type_url.contains("HttpRoute") { snap.http_routes.retain(|r| r.name != n); }
        else if resp.type_url.contains("GrpcRoute") { snap.grpc_routes.retain(|r| r.name != n); }
        else if resp.type_url.contains("StreamRoute") { snap.stream_routes.retain(|r| r.name != n); }
        else if resp.type_url.contains("BackendCluster") { snap.backends.retain(|b| b.name != n); }
        else if resp.type_url.contains("SecretMaterial") { snap.secrets.retain(|s| s.name != n); }
    }
}
