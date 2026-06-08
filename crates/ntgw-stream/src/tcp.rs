use std::borrow::Cow;
use std::io;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::OwnedReadHalf,
    net::{TcpListener, TcpStream},
    sync::watch,
    time::{Duration, Instant as TokioInstant, timeout},
};
use tracing::{debug, info, warn};

use ntgw_ir::{SelectedBackend, SelectedBackendRuntimeIds, SharedSnapshot};
use ntgw_observability::{
    AccessLogOptions, AccessLogRecord, SharedTrafficStats, TcpAdmissionController,
    TrafficObservationRef, TrafficRuntimeIds, UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT,
    current_timestamp, write_access_log,
};

use crate::{
    access_log::stream_access_log_state,
    normalize_tcp_proxy_buffer_bytes,
    pool::TcpConnectionPool,
    socket_addr,
    traffic::{ZERO_UPSTREAM_CONNECT_LATENCY_MS_BUCKETS, stream_route_kind_label},
};

const TLS_PREFACE_LIMIT: usize = 4096;
const TLS_PREFACE_READ_TIMEOUT: Duration = Duration::from_secs(1);
const TLS_PREFACE_READ_CHUNK: usize = 1024;

pub async fn bind(bind_addr: &str) -> Result<TcpListener> {
    TcpListener::bind(bind_addr).await.map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
pub async fn run_with_listener(
    snapshot: SharedSnapshot,
    listener_name: String,
    bind_addr: String,
    server: TcpListener,
    mut shutdown: watch::Receiver<bool>,
    is_tls_passthrough: bool,
    access_log: AccessLogOptions,
    traffic: SharedTrafficStats,
    admission: TcpAdmissionController,
    tcp_proxy_buffer_bytes: usize,
    idle_timeout: Option<Duration>,
    max_connection_age: Option<Duration>,
    pool: Arc<TcpConnectionPool>,
) -> Result<()> {
    info!(
        listener = %listener_name,
        bind = %bind_addr,
        tls_passthrough = is_tls_passthrough,
        "stream tcp listener started"
    );

    loop {
        tokio::select! {
                    _ = shutdown.changed() => {
                        info!(listener = %listener_name, "stream tcp listener stopping");
                        return Ok(());
                    }
                    accepted = server.accept() => {
                        let (downstream, peer) = accepted?;
                        let permit = match admission.try_acquire(&listener_name) {
                            Ok(permit) => permit,
                            Err(rejection) => {
                                debug!(
                                    listener = %listener_name,
                                    peer = %peer,
                                    scope = rejection.scope_label(),
                                    "stream tcp connection rejected due to overload"
                                );
                                let mut downstream = downstream;
                                let _ = downstream.shutdown().await;
                                continue;
                            }
                        };
                        let task_snapshot = snapshot.clone();
                        let task_listener_name = listener_name.clone();
                        let task_access_log = access_log.clone();
        let task_traffic = traffic.clone();
                        let task_pool = pool.clone();
                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Err(err) = handle_connection(
                                task_snapshot,
                                task_listener_name.clone(),
                                downstream,
                                is_tls_passthrough,
                                task_access_log,
                                task_traffic,
                                tcp_proxy_buffer_bytes,
                                idle_timeout,
                                max_connection_age,
                                task_pool,
                            )
                            .await
                            {
                                warn!(listener = %task_listener_name, peer = %peer, error = %err, "stream tcp connection failed");
                            }
                        });
                    }
                }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_connection(
    snapshot: SharedSnapshot,
    listener_name: String,
    mut downstream: TcpStream,
    is_tls_passthrough: bool,
    access_log: AccessLogOptions,
    traffic: SharedTrafficStats,
    tcp_proxy_buffer_bytes: usize,
    idle_timeout: Option<Duration>,
    max_connection_age: Option<Duration>,
    pool: Arc<TcpConnectionPool>,
) -> Result<()> {
    let started_at = std::time::Instant::now();
    let mut preface = Vec::new();
    let mut server_name = None;

    if is_tls_passthrough {
        preface = read_preface(&mut downstream).await?;
        server_name = crate::sni::extract_server_name(&preface);
    }

    let (access_log_state, selected, runtime_ids) = {
        let current = snapshot.read();
        let selected = current
            .select_stream_backend(&listener_name, server_name.as_deref())
            .ok_or_else(|| anyhow!("no stream route matched listener {listener_name}"))?;
        let runtime_ids = current.selected_backend_runtime_ids(&selected);
        let access_log_state = stream_access_log_state(&access_log, &selected, current.id.as_str());
        (access_log_state, selected, runtime_ids)
    };
    let client_addr = access_log_state
        .as_ref()
        .and_then(|_| downstream.peer_addr().ok());
    downstream.set_nodelay(true)?;
    let upstream_addr = socket_addr(&selected.backend.address, selected.backend.port);
    let connect_started = std::time::Instant::now();
    let _connect_latency_ms;
    let (upstream_result_from_pool, pool_counters_from_connect) = pool
        .get_connection(&selected.backend.address, selected.backend.port as u16)
        .await;
    let pool_counters = pool_counters_from_connect;
    let mut upstream = match upstream_result_from_pool {
        Ok(upstream) => {
            _connect_latency_ms =
                connect_started.elapsed().as_millis().min(u64::MAX as u128) as u64;
            upstream
        }
        Err(err) => {
            record_tcp_connect_failure(
                &traffic,
                TcpConnectFailure {
                    listener_name: &listener_name,
                    is_tls_passthrough,
                    selected: &selected,
                    runtime_ids,
                    started_at,
                    bytes_received: preface.len() as u64,
                    connect_latency_ms: connect_started.elapsed().as_millis().min(u64::MAX as u128)
                        as u64,
                },
            );
            return Err(err.into());
        }
    };

    upstream.set_nodelay(true)?;

    if !preface.is_empty() {
        upstream.write_all(&preface).await?;
    }

    debug!(
        listener = %listener_name,
        route = %selected.route_name,
        backend = %upstream_addr,
        "stream tcp backend selected"
    );

    let (upstream_read, upstream_write) = upstream.into_split();
    let outcome = proxy_stream_connection(
        downstream,
        upstream_read,
        upstream_write,
        tcp_proxy_buffer_bytes,
        idle_timeout,
        max_connection_age,
    )
    .await;
    let mut pool_error_count = 0u32;
    let proxy_succeeded = outcome.result.is_ok();
    if let Ok(upstream_reunited) =
        tokio::net::tcp::OwnedReadHalf::reunite(outcome.upstream_read, outcome.upstream_write)
    {
        if proxy_succeeded && pool.is_enabled() {
            // Only return healthy connections to the pool.
            // Check if the connection is still alive before pooling.
            let mut probe = [0u8; 1];
            let is_alive = match upstream_reunited.try_read(&mut probe) {
                Ok(0) => false,
                Ok(_) => false,
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => true,
                Err(_) => false,
            };
            if is_alive {
                pool.return_connection(
                    &selected.backend.address,
                    selected.backend.port as u16,
                    upstream_reunited,
                );
            }
        }
    } else {
        pool_error_count = 1;
    }

    let traffic_result = outcome.result;
    traffic.observe_ref(TrafficObservationRef {
        listener_name: listener_name.as_str(),
        protocol: if is_tls_passthrough {
            "TLS_PASSTHROUGH"
        } else {
            "TCP"
        },
        route_namespace: selected.route_namespace.as_str(),
        route_name: selected.route_name.as_str(),
        route_kind: stream_route_kind_label(&selected.route_kind),
        backend_name: selected.backend_name.as_str(),
        status: None,
        latency_ms: started_at.elapsed().as_millis().min(u64::MAX as u128) as u64,
        bytes_received: outcome.bytes_received,
        bytes_sent: outcome.bytes_sent,
        retry_attempts: 0,
        retried_success: false,
        upstream_pool_hits: pool_counters.hits,
        upstream_pool_misses: pool_counters.misses,
        upstream_peer_build_failures: pool_error_count,
        upstream_connect_latency_ms: 0,
        upstream_connect_latency_ms_max: 0,
        upstream_connect_latency_ms_buckets: &ZERO_UPSTREAM_CONNECT_LATENCY_MS_BUCKETS,
        response_flags: outcome.response_flags,
        runtime_ids: traffic_runtime_ids(runtime_ids),
    });
    if let Some(access_log_state) = access_log_state.as_ref() {
        let record = AccessLogRecord {
            event: if is_tls_passthrough {
                "tls_session".to_string()
            } else {
                "tcp_session".to_string()
            },
            timestamp: current_timestamp(),
            start_time_unix_ms: access_log_state.started_at_unix_ms,
            snapshot_version: access_log_state.snapshot_version.clone(),
            listener: Cow::Owned(listener_name.clone()),
            listener_runtime_id: runtime_ids.listener.map(|id| id.to_string()),
            protocol: Cow::Owned(if is_tls_passthrough {
                "TLS_PASSTHROUGH".to_string()
            } else {
                "TCP".to_string()
            }),
            client_ip: client_addr
                .map(|addr| addr.ip().to_string())
                .unwrap_or_else(|| "-".to_string()),
            host: server_name.as_deref().unwrap_or_default().to_string(),
            request_id: String::new(),
            route_namespace: Cow::Owned(selected.route_namespace.clone()),
            route_name: Cow::Owned(selected.route_name.clone()),
            route_kind: Cow::Owned(format!("{:?}", selected.route_kind)),
            route_runtime_id: runtime_ids.route.map(|id| id.to_string()),
            rule_runtime_id: runtime_ids.rule.map(|id| id.to_string()),
            backend: Cow::Owned(selected.backend_name.clone()),
            backend_runtime_id: runtime_ids.backend.map(|id| id.to_string()),
            endpoint_runtime_id: runtime_ids.endpoint.map(|id| id.to_string()),
            status: None,
            latency_ms: started_at.elapsed().as_millis(),
            bytes_sent: outcome.bytes_sent as usize,
            bytes_received: outcome.bytes_received as usize,
            retry_attempts: 0,
            response_flags: outcome.response_flags.to_string(),
            ..AccessLogRecord::default()
        };
        if let Err(err) = write_access_log(&access_log, &selected.route_annotations, &record) {
            warn!(listener = %listener_name, route = %selected.route_name, error = %err, "failed to emit stream tcp access log");
        }
    }
    traffic_result?;
    Ok(())
}

struct TcpConnectFailure<'a> {
    listener_name: &'a str,
    is_tls_passthrough: bool,
    selected: &'a SelectedBackend,
    runtime_ids: SelectedBackendRuntimeIds,
    started_at: std::time::Instant,
    bytes_received: u64,
    connect_latency_ms: u64,
}

fn record_tcp_connect_failure(traffic: &SharedTrafficStats, failure: TcpConnectFailure<'_>) {
    let upstream_connect_latency_ms_buckets = {
        let mut buckets = [0; UPSTREAM_CONNECT_LATENCY_MS_BUCKET_COUNT];
        let bucket = ntgw_observability::upstream_connect_latency_ms_bucket_index(
            failure.connect_latency_ms,
        );
        buckets[bucket] = 1;
        buckets
    };
    traffic.observe_ref(TrafficObservationRef {
        listener_name: failure.listener_name,
        protocol: if failure.is_tls_passthrough {
            "TLS_PASSTHROUGH"
        } else {
            "TCP"
        },
        route_namespace: failure.selected.route_namespace.as_str(),
        route_name: failure.selected.route_name.as_str(),
        route_kind: stream_route_kind_label(&failure.selected.route_kind),
        backend_name: failure.selected.backend_name.as_str(),
        status: None,
        latency_ms: failure
            .started_at
            .elapsed()
            .as_millis()
            .min(u64::MAX as u128) as u64,
        bytes_received: failure.bytes_received,
        bytes_sent: 0,
        retry_attempts: 0,
        retried_success: false,
        upstream_pool_hits: 0,
        upstream_pool_misses: 0,
        upstream_peer_build_failures: 0,
        upstream_connect_latency_ms: failure.connect_latency_ms,
        upstream_connect_latency_ms_max: failure.connect_latency_ms,
        upstream_connect_latency_ms_buckets: &upstream_connect_latency_ms_buckets,
        response_flags: "UF",
        runtime_ids: traffic_runtime_ids(failure.runtime_ids),
    });
}

fn traffic_runtime_ids(runtime_ids: SelectedBackendRuntimeIds) -> TrafficRuntimeIds {
    TrafficRuntimeIds {
        listener: runtime_ids.listener.map(|id| id.as_u64()),
        route: runtime_ids.route.map(|id| id.as_u64()),
        backend: runtime_ids.backend.map(|id| id.as_u64()),
    }
}

#[derive(Debug, Default)]
struct TcpProxyOutcome {
    bytes_received: u64,
    bytes_sent: u64,
    response_flags: &'static str,
}

#[derive(Debug)]
struct TcpProxyResult {
    result: anyhow::Result<TcpProxyOutcome>,
    upstream_read: tokio::net::tcp::OwnedReadHalf,
    upstream_write: tokio::net::tcp::OwnedWriteHalf,
    bytes_received: u64,
    bytes_sent: u64,
    response_flags: &'static str,
}

enum ProxyEvent {
    DownstreamRead(usize),
    UpstreamRead(usize),
    DownstreamClosed,
    UpstreamClosed,
    DownstreamReset,
    UpstreamReset,
    IdleTimeout,
    MaxConnectionAgeReached,
}

async fn proxy_stream_connection(
    downstream: TcpStream,
    mut upstream_read: tokio::net::tcp::OwnedReadHalf,
    mut upstream_write: tokio::net::tcp::OwnedWriteHalf,
    tcp_proxy_buffer_bytes: usize,
    idle_timeout: Option<Duration>,
    max_connection_age: Option<Duration>,
) -> TcpProxyResult {
    let (mut downstream_read, mut downstream_write) = downstream.into_split();
    let mut downstream_open = true;
    let mut upstream_open = true;
    let buffer_bytes = normalize_tcp_proxy_buffer_bytes(tcp_proxy_buffer_bytes);
    let mut downstream_buf = vec![0; buffer_bytes];
    let mut upstream_buf = vec![0; buffer_bytes];
    let mut outcome = TcpProxyOutcome::default();
    let max_age_deadline = max_connection_age.map(|age| TokioInstant::now() + age);
    let mut proxy_error: Option<anyhow::Error> = None;

    loop {
        if !downstream_open && !upstream_open {
            break;
        }

        let event = match next_proxy_event(
            &mut downstream_read,
            &mut upstream_read,
            &mut downstream_buf,
            &mut upstream_buf,
            downstream_open,
            upstream_open,
            idle_timeout,
            max_age_deadline,
        )
        .await
        {
            Ok(event) => event,
            Err(err) => {
                outcome.response_flags = "NE";
                proxy_error = Some(err);
                break;
            }
        };

        let mut should_break = false;
        match event {
            ProxyEvent::DownstreamRead(read) => {
                if let Err(err) = upstream_write.write_all(&downstream_buf[..read]).await {
                    if is_tcp_connection_closed(&err) {
                        outcome.response_flags = "UC";
                        let _ = downstream_write.shutdown().await;
                        should_break = true;
                    } else {
                        outcome.response_flags = "UE";
                        proxy_error = Some(err.into());
                        should_break = true;
                    }
                }
                outcome.bytes_received = outcome.bytes_received.saturating_add(read as u64);
            }
            ProxyEvent::UpstreamRead(read) => {
                if let Err(err) = downstream_write.write_all(&upstream_buf[..read]).await {
                    if is_tcp_connection_closed(&err) {
                        outcome.response_flags = "DC";
                        let _ = upstream_write.shutdown().await;
                        should_break = true;
                    } else {
                        outcome.response_flags = "DE";
                        proxy_error = Some(err.into());
                        should_break = true;
                    }
                }
                outcome.bytes_sent = outcome.bytes_sent.saturating_add(read as u64);
            }
            ProxyEvent::DownstreamClosed => {
                downstream_open = false;
                let _ = upstream_write.shutdown().await;
            }
            ProxyEvent::UpstreamClosed => {
                upstream_open = false;
                let _ = downstream_write.shutdown().await;
            }
            ProxyEvent::DownstreamReset => {
                outcome.response_flags = "DC";
                let _ = upstream_write.shutdown().await;
                should_break = true;
            }
            ProxyEvent::UpstreamReset => {
                outcome.response_flags = "UC";
                let _ = downstream_write.shutdown().await;
                should_break = true;
            }
            ProxyEvent::IdleTimeout => {
                outcome.response_flags = "IT";
                let _ = downstream_write.shutdown().await;
                let _ = upstream_write.shutdown().await;
                should_break = true;
            }
            ProxyEvent::MaxConnectionAgeReached => {
                outcome.response_flags = "MC";
                let _ = downstream_write.shutdown().await;
                let _ = upstream_write.shutdown().await;
                should_break = true;
            }
        }
        if should_break {
            break;
        }
    }

    let outcome_bytes_received = outcome.bytes_received;
    let outcome_bytes_sent = outcome.bytes_sent;
    let outcome_response_flags = outcome.response_flags;

    TcpProxyResult {
        result: match proxy_error {
            Some(err) => Err(err),
            None => Ok(outcome),
        },
        upstream_read,
        upstream_write,
        bytes_received: outcome_bytes_received,
        bytes_sent: outcome_bytes_sent,
        response_flags: outcome_response_flags,
    }
}

#[allow(clippy::too_many_arguments)]
async fn next_proxy_event(
    downstream: &mut OwnedReadHalf,
    upstream: &mut OwnedReadHalf,
    downstream_buf: &mut [u8],
    upstream_buf: &mut [u8],
    downstream_open: bool,
    upstream_open: bool,
    idle_timeout: Option<Duration>,
    max_age_deadline: Option<TokioInstant>,
) -> Result<ProxyEvent> {
    let next_read = async {
        tokio::select! {
            read = downstream.read(downstream_buf), if downstream_open => {
                match read {
                    Ok(0) => Ok(ProxyEvent::DownstreamClosed),
                    Ok(read) => Ok(ProxyEvent::DownstreamRead(read)),
                    Err(err) if is_tcp_connection_closed(&err) => Ok(ProxyEvent::DownstreamReset),
                    Err(err) => Err(err.into()),
                }
            }
            read = upstream.read(upstream_buf), if upstream_open => {
                match read {
                    Ok(0) => Ok(ProxyEvent::UpstreamClosed),
                    Ok(read) => Ok(ProxyEvent::UpstreamRead(read)),
                    Err(err) if is_tcp_connection_closed(&err) => Ok(ProxyEvent::UpstreamReset),
                    Err(err) => Err(err.into()),
                }
            }
            else => unreachable!("next_proxy_event requires an open tcp half"),
        }
    };

    match (idle_timeout, max_age_deadline) {
        (Some(idle_timeout), Some(max_age_deadline)) => {
            let age_limit = tokio::time::sleep_until(max_age_deadline);
            tokio::pin!(age_limit);
            tokio::select! {
                _ = &mut age_limit => Ok(ProxyEvent::MaxConnectionAgeReached),
                read = timeout(idle_timeout, next_read) => match read {
                    Ok(result) => result,
                    Err(_) => Ok(ProxyEvent::IdleTimeout),
                }
            }
        }
        (Some(idle_timeout), None) => match timeout(idle_timeout, next_read).await {
            Ok(result) => result,
            Err(_) => Ok(ProxyEvent::IdleTimeout),
        },
        (None, Some(max_age_deadline)) => {
            let age_limit = tokio::time::sleep_until(max_age_deadline);
            tokio::pin!(age_limit);
            tokio::select! {
                _ = &mut age_limit => Ok(ProxyEvent::MaxConnectionAgeReached),
                result = next_read => result,
            }
        }
        (None, None) => next_read.await,
    }
}

fn is_tcp_connection_closed(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::NotConnected
            | io::ErrorKind::UnexpectedEof
    )
}

async fn read_preface(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(TLS_PREFACE_READ_CHUNK);

    loop {
        if buf.len() >= TLS_PREFACE_LIMIT {
            break;
        }

        let mut chunk = vec![0; (TLS_PREFACE_LIMIT - buf.len()).min(TLS_PREFACE_READ_CHUNK)];
        let read = match timeout(TLS_PREFACE_READ_TIMEOUT, stream.read(&mut chunk)).await {
            Ok(Ok(read)) => read,
            Ok(Err(err)) => return Err(err.into()),
            Err(_) => return Err(anyhow!("timed out reading client preface")),
        };
        if read == 0 {
            break;
        }

        buf.extend_from_slice(&chunk[..read]);

        match crate::sni::tls_record_len(&buf) {
            Some(expected) if buf.len() >= expected => break,
            Some(_) => continue,
            None if buf.len() < 5 => continue,
            None => break,
        }
    }

    if buf.is_empty() {
        return Err(anyhow!("connection closed before client preface"));
    }

    Ok(buf)
}

#[cfg(test)]
mod tests;
