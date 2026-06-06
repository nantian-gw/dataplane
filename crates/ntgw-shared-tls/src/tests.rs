use std::{collections::BTreeMap, net::TcpListener as StdTcpListener, pin::Pin};

use ntgw_ir::{
    BackendCluster, BackendEndpoint, BackendRef, HttpMatch, HttpRoute, HttpRule, Listener,
    ParentRef, SecretMaterial, SharedSnapshot, Snapshot, StreamMatch, StreamRoute, StreamRule,
    TlsConfig, TlsRouteMode,
};
use anyhow::{Context, Result};
use pingora::{
    protocols::tls::SslStream,
    tls::ssl::{SslConnector, SslMethod, SslVerifyMode},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{timeout, Duration},
};

const VALID_SERVER_CERT_PEM: &str = include_str!("../../../testdata/backendtls/server-san.crt");
const VALID_SERVER_KEY_PEM: &str = include_str!("../../../testdata/backendtls/server-san.key");

#[path = "tests/listener_plan.rs"]
mod listener_plan_tests;

fn example_secret_material(name: &str) -> SecretMaterial {
    SecretMaterial {
        namespace: "default".to_string(),
        name: name.to_string(),
        cert_pem: VALID_SERVER_CERT_PEM.to_string(),
        key_pem: VALID_SERVER_KEY_PEM.to_string(),
    }
}

fn wildcard_secret_material(name: &str) -> SecretMaterial {
    SecretMaterial {
        namespace: "default".to_string(),
        name: name.to_string(),
        cert_pem: VALID_SERVER_CERT_PEM.replace("server-san.example", "*.example.org"),
        key_pem: VALID_SERVER_KEY_PEM.to_string(),
    }
}

fn build_client_hello(host: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&[0x03, 0x03]);
    body.extend_from_slice(&[0; 32]);
    body.push(0);
    body.extend_from_slice(&[0x00, 0x02, 0x00, 0x2f]);
    body.extend_from_slice(&[0x01, 0x00]);

    let mut server_name = Vec::new();
    server_name.extend_from_slice(&(host.len() as u16 + 3).to_be_bytes());
    server_name.push(0);
    server_name.extend_from_slice(&(host.len() as u16).to_be_bytes());
    server_name.extend_from_slice(host.as_bytes());

    let mut extensions = Vec::new();
    extensions.extend_from_slice(&[0x00, 0x00]);
    extensions.extend_from_slice(&(server_name.len() as u16).to_be_bytes());
    extensions.extend_from_slice(&server_name);

    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend_from_slice(&extensions);

    let mut handshake = vec![
        0x01,
        ((body.len() >> 16) & 0xff) as u8,
        ((body.len() >> 8) & 0xff) as u8,
        (body.len() & 0xff) as u8,
    ];
    handshake.extend_from_slice(&body);

    let mut record = Vec::new();
    record.extend_from_slice(&[0x16, 0x03, 0x01]);
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend_from_slice(&handshake);
    record
}

fn free_tcp_port() -> u16 {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("free port bind");
    listener.local_addr().expect("free port addr").port()
}

async fn read_http_headers<S>(stream: &mut S) -> Result<String>
where
    S: AsyncRead + Unpin,
{
    let mut buf = Vec::new();
    loop {
        let byte = timeout(Duration::from_secs(2), stream.read_u8()).await??;
        buf.push(byte);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    Ok(String::from_utf8(buf)?)
}

async fn read_http_response<S>(stream: &mut S) -> Result<String>
where
    S: AsyncRead + Unpin,
{
    let headers = read_http_headers(stream).await?;
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or_default();
    let mut body = vec![0; content_length];
    if content_length > 0 {
        stream.read_exact(&mut body).await?;
    }
    let mut raw = headers.into_bytes();
    raw.extend_from_slice(&body);
    Ok(String::from_utf8(raw)?)
}

async fn https_request(bind: &str, sni: &str, host: &str, path: &str) -> Result<String> {
    let tcp = TcpStream::connect(bind).await?;
    let mut connector = SslConnector::builder(SslMethod::tls()).context("ssl connector")?;
    connector.set_verify(SslVerifyMode::NONE);
    let ssl = connector
        .build()
        .configure()
        .context("ssl configure")?
        .into_ssl(sni)
        .context("ssl create")?;
    let mut stream = SslStream::new(ssl, tcp).context("ssl stream")?;
    Pin::new(&mut stream)
        .connect()
        .await
        .context("ssl connect")?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await?;
    read_http_response(&mut stream).await
}

fn shared_tls_snapshot(
    gateway_port: u16,
    http_backend_port: u16,
    stream_backend_port: u16,
) -> SharedSnapshot {
    let shared = Snapshot::shared();
    *shared.write() = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/https".to_string(),
                address: "127.0.0.1".to_string(),
                addresses: vec!["127.0.0.1".to_string()],
                port: gateway_port as u32,
                protocol: "LISTENER_PROTOCOL_HTTPS".to_string(),
                attached_routes: vec!["default/http-route".to_string()],
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: false,
                    secret_refs: vec!["default/example-cert".to_string()],
                    sni_hosts: vec![],
                    min_version: "1.2".to_string(),
                    max_version: "1.3".to_string(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
            Listener {
                name: "default/gw/tls".to_string(),
                address: "127.0.0.1".to_string(),
                addresses: vec!["127.0.0.1".to_string()],
                port: gateway_port as u32,
                protocol: "LISTENER_PROTOCOL_TLS_PASSTHROUGH".to_string(),
                attached_routes: vec!["default/tls-route".to_string()],
                tls: Some(TlsConfig {
                    enabled: true,
                    passthrough: true,
                    secret_refs: vec![],
                    sni_hosts: vec![],
                    min_version: String::new(),
                    max_version: String::new(),
                    frontend_validation: None,
                }),
                ..Listener::default()
            },
        ],
        http_routes: vec![HttpRoute {
            name: "http-route".to_string(),
            namespace: "default".to_string(),
            hostnames: Vec::new(),
            parent_refs: vec![ParentRef {
                namespace: "default".to_string(),
                name: "gw".to_string(),
                section_name: String::new(),
                port: gateway_port as u32,
                ..ParentRef::default()
            }],
            rules: vec![HttpRule {
                name: String::new(),
                matches: vec![HttpMatch {
                    path: "/".to_string(),
                    path_type: "Exact".to_string(),
                    ..HttpMatch::default()
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "http-backend".to_string(),
                    port: http_backend_port as u32,
                    ..BackendRef::default()
                }],
                ..HttpRule::default()
            }],
            annotations: BTreeMap::new(),
        }],
        stream_routes: vec![StreamRoute {
            name: "tls-route".to_string(),
            namespace: "default".to_string(),
            kind: "ROUTE_KIND_TLS".to_string(),
            parent_refs: Vec::new(),
            rules: vec![StreamRule {
                name: String::new(),
                matches: vec![StreamMatch {
                    port: gateway_port as u32,
                    sni_hostname: "passthrough.example.com".to_string(),
                    mode: TlsRouteMode::default(),
                }],
                backend_refs: vec![BackendRef {
                    namespace: "default".to_string(),
                    name: "stream-backend".to_string(),
                    port: stream_backend_port as u32,
                    ..BackendRef::default()
                }],
            }],
            annotations: BTreeMap::new(),
        }],
        backends: vec![
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("http-backend:{http_backend_port}"),
                namespace: "default".to_string(),
                protocol: "HTTP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: http_backend_port as u32,
                    healthy: true,
                }],
                wasm_plugin: None,
            },
            BackendCluster {
                ai_service: None,
                token_policy: None,
                name: format!("stream-backend:{stream_backend_port}"),
                namespace: "default".to_string(),
                protocol: "TCP".to_string(),
                endpoints: vec![BackendEndpoint {
                    address: "127.0.0.1".to_string(),
                    port: stream_backend_port as u32,
                    healthy: true,
                }],
                wasm_plugin: None,
            },
        ],
        secrets: vec![example_secret_material("example-cert")],
        ..Snapshot::default()
    };
    shared.write().rebuild_runtime_indexes();
    shared
}

#[path = "tests/dispatch.rs"]
mod dispatch;

#[path = "tests/runtime.rs"]
mod runtime;
