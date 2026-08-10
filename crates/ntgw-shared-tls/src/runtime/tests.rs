use std::{net::TcpListener as StdTcpListener, pin::Pin};

use anyhow::Result;
use ntgw_http::{
    AccessLogOptions, RuntimeOptions as HttpRuntimeOptions, SessionPersistenceOptions,
    build_http_app,
};
use ntgw_ir::Snapshot;
use ntgw_ir::{Listener, TlsConfig};
use ntgw_observability::{
    HttpCircuitBreakerController, HttpRateLimitController, RetryBudgetController,
    SharedTrafficStats,
};
use pingora::{
    protocols::tls::SslStream,
    tls::{
        pkey::PKey,
        ssl::{SslConnector, SslMethod, SslVerifyMode},
        x509::X509,
    },
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::listener_plan::{SharedTlsIdentity, TerminateSurface};

use super::*;
use std::sync::Arc;
use std::time::Duration;
use ntgw_http::cache::{CacheManager, CacheOptions};
use ntgw_http::proxy::GatewayProxyOptions;
use ntgw_observability::{HttpAdmissionController, HttpAdmissionOptions, OverloadStats};


const SERVER_CERT_PEM: &str = include_str!("../../../../testdata/backendtls/server-san.crt");
const SERVER_KEY_PEM: &str = include_str!("../../../../testdata/backendtls/server-san.key");
const CLIENT_CERT_PEM: &str = include_str!("../../../../testdata/tls/client.crt");
const CLIENT_KEY_PEM: &str = include_str!("../../../../testdata/tls/client.key");

include!("tests/binds.rs");
include!("tests/desired_plan.rs");
include!("tests/frontend_validation.rs");
include!("tests/handshake.rs");
include!("tests/passthrough_selection.rs");

async fn run_frontend_validation_handshake(
    mode: &str,
    client_ca_bundle_pem: &str,
    present_client_cert: bool,
) -> Result<(Result<(), String>, Result<(), String>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let terminate = TerminateSurface {
        listener_names: vec!["default/gw/https".to_string()],
        identities: vec![SharedTlsIdentity {
            secret_ref: "default/server".to_string(),
            cert_pem: SERVER_CERT_PEM.to_string(),
            key_pem: SERVER_KEY_PEM.to_string(),
            match_names: vec!["server-san.example".to_string()],
        }],
        frontend_validation_mode: Some(mode.to_string()),
        client_ca_bundle_pem: Some(client_ca_bundle_pem.to_string()),
    };

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.map_err(|err| err.to_string())?;
        let stream = L4Stream::from(stream);
        let mut tls_stream = terminate_tls(stream, &terminate)
            .await
            .map_err(|err| err.to_string())?;
        tls_stream
            .write_all(b"ok")
            .await
            .map_err(|err| err.to_string())?;
        Ok::<(), String>(())
    });

    let tcp = TcpStream::connect(addr).await?;
    let mut connector = SslConnector::builder(SslMethod::tls())?;
    connector.set_verify(SslVerifyMode::NONE);
    if present_client_cert {
        let cert = X509::from_pem(CLIENT_CERT_PEM.as_bytes())?;
        let key = PKey::private_key_from_pem(CLIENT_KEY_PEM.as_bytes())?;
        connector.set_certificate(&cert)?;
        connector.set_private_key(&key)?;
        connector.check_private_key()?;
    }
    let ssl = connector
        .build()
        .configure()?
        .into_ssl("server-san.example")?;
    let mut client_stream = SslStream::new(ssl, tcp)?;
    let client_result = async {
        Pin::new(&mut client_stream)
            .connect()
            .await
            .map_err(|err| err.to_string())?;
        let mut response = [0_u8; 2];
        client_stream
            .read_exact(&mut response)
            .await
            .map_err(|err| err.to_string())?;
        Ok::<(), String>(())
    }
    .await;
    let server_result = server.await?;

    Ok((server_result, client_result))
}

fn free_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .expect("free port bind")
        .local_addr()
        .expect("free port addr")
        .port()
}
