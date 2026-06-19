use std::{net::SocketAddr, sync::Arc, time::Instant};

use anyhow::{Result, anyhow};
use tokio::{net::UdpSocket, sync::watch, time::Duration};
use tracing::{debug, info, warn};

use ntgw_ir::SharedSnapshot;
use ntgw_observability::{
    AccessLogOptions, SharedTrafficStats, SharedUdpSessionStats, UdpAdmissionController,
    UdpAdmissionPermit,
};

#[cfg(test)]
use crate::ephemeral_bind_addr;
use crate::{access_log::stream_access_log_state, socket_addr};

mod dispatcher;
mod session;
mod telemetry;

use self::{
    dispatcher::UdpDatagramDispatcher,
    session::{UdpSessionRegistry, UdpSessionTask},
    telemetry::record_udp_datagram,
};
pub(crate) use self::{
    dispatcher::{
        UDP_DISPATCHER_QUEUE_CAPACITY, UDP_DISPATCHER_WORKERS, udp_dispatcher_worker_index,
    },
    session::{UDP_SESSION_SHARDS, UdpSessionKey, udp_session_shard_index},
};

pub async fn bind(bind_addr: &str) -> Result<UdpSocket> {
    UdpSocket::bind(bind_addr).await.map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
pub async fn run_with_socket(
    snapshot: SharedSnapshot,
    listener_name: String,
    bind_addr: String,
    socket: Arc<UdpSocket>,
    mut shutdown: watch::Receiver<bool>,
    access_log: AccessLogOptions,
    traffic: SharedTrafficStats,
    admission: UdpAdmissionController,
    udp_sessions: SharedUdpSessionStats,
    udp_response_idle_timeout: Duration,
) -> Result<()> {
    let listener_name = Arc::<str>::from(listener_name);
    let mut buf = vec![0; 65_535];
    let sessions = UdpSessionRegistry::with_stats(udp_sessions);
    let dispatcher =
        UdpDatagramDispatcher::new(Arc::clone(&socket), sessions, udp_response_idle_timeout);
    info!(listener = %listener_name, bind = %bind_addr, "stream udp listener started");

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                info!(listener = %listener_name, "stream udp listener stopping");
                return Ok(());
            }
            recv = socket.recv_from(&mut buf) => {
                let (size, client_addr) = recv?;
                let permit = match admission.try_acquire(listener_name.as_ref()) {
                    Ok(permit) => permit,
                    Err(rejection) => {
                        debug!(
                            listener = %listener_name,
                            client = %client_addr,
                            scope = rejection.scope_label(),
                            "stream udp datagram dropped due to overload"
                        );
                        continue;
                    }
                };
                let payload = buf[..size].to_vec();
                let task = match build_udp_session_task(
                    &snapshot,
                    Arc::clone(&listener_name),
                    client_addr,
                    payload,
                    &access_log,
                    traffic.clone(),
                    permit,
                ) {
                    Ok(task) => task,
                    Err(err) => {
                        warn!(listener = %listener_name, client = %client_addr, error = %err, "stream udp datagram failed");
                        continue;
                    }
                };
                if let Err(err) = dispatcher.dispatch(task).await {
                    warn!(listener = %listener_name, client = %client_addr, error = %err, "stream udp datagram failed");
                }
            }
        }
    }
}

fn build_udp_session_task(
    snapshot: &SharedSnapshot,
    listener_name: Arc<str>,
    client_addr: SocketAddr,
    payload: Vec<u8>,
    access_log: &AccessLogOptions,
    traffic: SharedTrafficStats,
    permit: UdpAdmissionPermit,
) -> Result<UdpSessionTask> {
    let current = snapshot.load();
    let selected = current
        .select_stream_backend(listener_name.as_ref(), None)
        .ok_or_else(|| anyhow!("no stream route matched listener {listener_name}"))?;
    let runtime_ids = current.selected_backend_runtime_ids(&selected);
    let access_log_state = stream_access_log_state(access_log, &selected, current.id.as_str());
    let access_log = access_log_state.as_ref().map(|_| access_log.clone());
    let upstream_addr = socket_addr(&selected.backend.address, selected.backend.port).parse()?;
    Ok(UdpSessionTask {
        listener_name,
        selected,
        runtime_ids,
        upstream_addr,
        client_addr,
        payload,
        access_log,
        access_log_state,
        traffic,
        started_at: Instant::now(),
        _permit: permit,
    })
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
async fn proxy_datagram(
    snapshot: SharedSnapshot,
    listener_name: String,
    downstream: Arc<UdpSocket>,
    client_addr: SocketAddr,
    payload: Vec<u8>,
    access_log: AccessLogOptions,
    traffic: SharedTrafficStats,
    udp_response_idle_timeout: Duration,
) -> Result<()> {
    let started_at = Instant::now();
    let (access_log_state, selected, runtime_ids) = {
        let current = snapshot.load();
        let selected = current
            .select_stream_backend(&listener_name, None)
            .ok_or_else(|| anyhow!("no stream route matched listener {listener_name}"))?;
        let runtime_ids = current.selected_backend_runtime_ids(&selected);
        let access_log_state = stream_access_log_state(&access_log, &selected, current.id.as_str());
        (access_log_state, selected, runtime_ids)
    };
    let upstream_addr = socket_addr(&selected.backend.address, selected.backend.port);
    let upstream = UdpSocket::bind(ephemeral_bind_addr(&selected.backend.address)).await?;
    upstream.connect(&upstream_addr).await?;
    upstream.send(&payload).await?;

    debug!(
        listener = %listener_name,
        route = %selected.route_name,
        backend = %upstream_addr,
        "stream udp backend selected"
    );

    let mut response = vec![0; 65_535];
    let mut total_response_bytes = 0usize;
    loop {
        match tokio::time::timeout(udp_response_idle_timeout, upstream.recv(&mut response)).await {
            Ok(Ok(size)) => {
                total_response_bytes += size;
                downstream.send_to(&response[..size], client_addr).await?;
            }
            Ok(Err(err)) => return Err(err.into()),
            Err(_) => {
                record_udp_datagram(
                    &traffic,
                    access_log_state.as_ref().map(|_| &access_log),
                    access_log_state.as_ref(),
                    &selected,
                    runtime_ids,
                    client_addr,
                    payload.len(),
                    total_response_bytes,
                    started_at,
                );
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests;
