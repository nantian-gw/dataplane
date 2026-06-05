use aeg_ir::{SharedSnapshot, TlsRouteMode};
use anyhow::{anyhow, Result};
use pingora::protocols::l4::stream::Stream as L4Stream;
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub(crate) async fn proxy_passthrough(
    mut downstream: L4Stream,
    snapshot: &SharedSnapshot,
    listener_name: &str,
    server_name: Option<&str>,
) -> Result<()> {
    let selected = {
        let current = snapshot.read();
        current
            .select_tls_stream_backend(listener_name, server_name, TlsRouteMode::Passthrough)
            .ok_or_else(|| anyhow!("no tls passthrough route matched listener {listener_name}"))?
    };
    let upstream_addr = format!("{}:{}", selected.backend.address, selected.backend.port);
    let mut upstream = TcpStream::connect(&upstream_addr).await?;
    let _ = copy_bidirectional(&mut downstream, &mut upstream).await?;
    Ok(())
}

pub(crate) async fn proxy_terminated_stream<T>(
    mut downstream: T,
    snapshot: &SharedSnapshot,
    listener_name: &str,
    server_name: Option<&str>,
) -> Result<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let selected = {
        let current = snapshot.read();
        current
            .select_tls_stream_backend(listener_name, server_name, TlsRouteMode::Terminate)
            .ok_or_else(|| anyhow!("no terminated tls route matched listener {listener_name}"))?
    };
    let upstream_addr = format!("{}:{}", selected.backend.address, selected.backend.port);
    let mut upstream = TcpStream::connect(&upstream_addr).await?;
    let _ = copy_bidirectional(&mut downstream, &mut upstream).await?;
    Ok(())
}
