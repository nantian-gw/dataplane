use anyhow::anyhow;
use crate::SharedTlsError;
use ntgw_ir::{SharedSnapshot, TlsRouteMode};
use pingora::protocols::l4::stream::Stream as L4Stream;
use tokio::{
    io::{AsyncRead, AsyncWrite, copy_bidirectional},
    net::TcpStream,
};

pub(crate) async fn proxy_passthrough(
    mut downstream: L4Stream,
    snapshot: &SharedSnapshot,
    listener_name: &str,
    server_name: Option<&str>,
) -> Result<(), SharedTlsError> {
    let selected = {
        let current = snapshot.load();
        current
            .select_tls_stream_backend(listener_name, server_name, TlsRouteMode::Passthrough)
            .ok_or_else(|| SharedTlsError::Certificate(anyhow!("no tls passthrough route matched listener {listener_name}")))?
    };
    let upstream_addr = format!("{}:{}", selected.backend.address, selected.backend.port);
    let mut upstream = TcpStream::connect(&upstream_addr).await?;
    if let Err(e) = copy_bidirectional(&mut downstream, &mut upstream).await {
        tracing::debug!(error = %e, "TLS copy_bidirectional closed");
    }
    Ok(())
}

pub(crate) async fn proxy_terminated_stream<T>(
    mut downstream: T,
    snapshot: &SharedSnapshot,
    listener_name: &str,
    server_name: Option<&str>,
) -> Result<(), SharedTlsError>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let selected = {
        let current = snapshot.load();
        current
            .select_tls_stream_backend(listener_name, server_name, TlsRouteMode::Terminate)
            .ok_or_else(|| SharedTlsError::Certificate(anyhow!("no terminated tls route matched listener {listener_name}")))?
    };
    let upstream_addr = format!("{}:{}", selected.backend.address, selected.backend.port);
    let mut upstream = TcpStream::connect(&upstream_addr).await?;
    if let Err(e) = copy_bidirectional(&mut downstream, &mut upstream).await {
        tracing::debug!(error = %e, "TLS copy_bidirectional closed");
    }
    Ok(())
}
