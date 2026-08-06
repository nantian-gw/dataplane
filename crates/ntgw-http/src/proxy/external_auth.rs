use std::{collections::BTreeMap, collections::BTreeSet, collections::HashMap, str};

use bytes::{Bytes, BytesMut};
use http::header::{AUTHORIZATION, CONTENT_LENGTH, HOST, HeaderName};
use ntgw_ir::{BackendEndpoint, ExternalAuthFilter, Filter};
use ntgw_proto::envoy::service::auth::v3::{
    AttributeContext, CheckRequest, DeniedHttpResponse, attribute_context,
    authorization_client::AuthorizationClient, check_response,
};
use pingora::http::{RequestHeader, ResponseHeader};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tonic::transport::Channel;
use tracing::warn;

pub(crate) enum ExternalAuthDecision {
    Allow(BTreeMap<String, Vec<String>>),
    Deny(Box<ResponseHeader>, Bytes),
}

pub(crate) fn external_auth_filter(filters: &[Filter]) -> Option<&ExternalAuthFilter> {
    filters
        .iter()
        .find_map(|filter| filter.external_auth.as_ref())
}

pub(crate) async fn run_external_auth(
    request: &RequestHeader,
    auth: &ExternalAuthFilter,
    endpoint: &BackendEndpoint,
    body: Option<&Bytes>,
) -> pingora::Result<ExternalAuthDecision> {
    if auth.protocol.eq_ignore_ascii_case("GRPC") {
        return run_grpc_external_auth(request, auth, endpoint, body).await;
    }

    warn!(
        address = %endpoint.address,
        port = endpoint.port,
        "external auth is using plaintext TCP without TLS — authentication tokens may be exposed; configure TLS for the auth backend"
    );

    let address = format!("{}:{}", endpoint.address, endpoint.port);
    let mut stream = TcpStream::connect(address)
        .await
        .map_err(external_auth_error)?;
    let request = build_auth_request(request, auth, body)?;
    stream
        .write_all(&request)
        .await
        .map_err(external_auth_error)?;
    let response = read_auth_response(&mut stream).await?;
    if (200..=299).contains(&response.status) {
        return Ok(ExternalAuthDecision::Allow(allowed_response_headers(
            &response.headers,
            &auth.http.allowed_response_headers,
        )));
    }

    let mut header = ResponseHeader::build(response.status, None)?;
    header.insert_header(CONTENT_LENGTH, response.body.len().to_string())?;
    Ok(ExternalAuthDecision::Deny(
        Box::new(header),
        response.body.freeze(),
    ))
}

fn build_auth_request(
    request: &RequestHeader,
    auth: &ExternalAuthFilter,
    body: Option<&Bytes>,
) -> pingora::Result<Vec<u8>> {
    let body = body.map(Bytes::as_ref).unwrap_or_default();
    let path = if auth.http.path.is_empty() {
        "/"
    } else {
        auth.http.path.as_str()
    };
    let mut out = format!("{} {path} HTTP/1.1\r\n", request.method.as_str());
    out.push_str("Host: external-auth\r\n");
    out.push_str("Connection: close\r\n");
    out.push_str(&format!("Content-Length: {}\r\n", body.len()));
    write_request_header_values(&mut out, request, &AUTHORIZATION);
    for name in &auth.http.allowed_headers {
        let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        if header_name == AUTHORIZATION {
            continue;
        }
        write_request_header_values(&mut out, request, &header_name);
    }
    out.push_str("\r\n");
    let mut out = out.into_bytes();
    out.extend_from_slice(body);
    Ok(out)
}

async fn run_grpc_external_auth(
    request: &RequestHeader,
    auth: &ExternalAuthFilter,
    endpoint: &BackendEndpoint,
    body: Option<&Bytes>,
) -> pingora::Result<ExternalAuthDecision> {
    let address = format!("http://{}:{}", endpoint.address, endpoint.port);
    let channel = Channel::from_shared(address)
        .map_err(|error| grpc_external_auth_protocol_error(error.to_string()))?
        .connect()
        .await
        .map_err(grpc_external_auth_transport_error)?;
    let mut client = AuthorizationClient::new(channel);
    let response = client
        .check(CheckRequest {
            attributes: Some(build_grpc_attribute_context(request, auth, body)),
        })
        .await
        .map_err(grpc_external_auth_status_error)?
        .into_inner();

    match response.http_response {
        Some(check_response::HttpResponse::OkResponse(_)) => {
            if response
                .status
                .as_ref()
                .is_some_and(|status| status.code != 0)
            {
                return Err(grpc_external_auth_protocol_error("non-OK ext_authz status"));
            }
            Ok(ExternalAuthDecision::Allow(BTreeMap::new()))
        }
        Some(check_response::HttpResponse::DeniedResponse(denied)) => grpc_denied_response(denied),
        None if response
            .status
            .as_ref()
            .is_some_and(|status| status.code == 0) =>
        {
            Ok(ExternalAuthDecision::Allow(BTreeMap::new()))
        }
        None => Err(grpc_external_auth_protocol_error(
            "ext_authz response missing HTTP decision",
        )),
    }
}

fn build_grpc_attribute_context(
    request: &RequestHeader,
    auth: &ExternalAuthFilter,
    body: Option<&Bytes>,
) -> AttributeContext {
    AttributeContext {
        request: Some(attribute_context::Request {
            http: Some(attribute_context::HttpRequest {
                method: request.method.as_str().to_string(),
                path: request
                    .uri
                    .path_and_query()
                    .map(|path| path.as_str().to_string())
                    .unwrap_or_else(|| request.uri.path().to_string()),
                host: request_host(request),
                headers: grpc_request_headers(request, &auth.grpc.allowed_headers),
                body: body.map(|body| body.to_vec()).unwrap_or_default(),
                ..attribute_context::HttpRequest::default()
            }),
        }),
    }
}

fn request_host(request: &RequestHeader) -> String {
    request
        .headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .or_else(|| request.uri.host().map(str::to_string))
        .unwrap_or_default()
}

fn grpc_request_headers(request: &RequestHeader, allowed: &[String]) -> HashMap<String, String> {
    let allowed: BTreeSet<String> = allowed
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let mut out: HashMap<String, String> = HashMap::new();
    for (name, value) in request.headers.iter() {
        let name = name.as_str().to_ascii_lowercase();
        if !allowed.is_empty() && !allowed.contains(&name) {
            continue;
        }
        let Ok(value) = value.to_str() else {
            continue;
        };
        out.entry(name)
            .and_modify(|existing| {
                existing.push(',');
                existing.push_str(value);
            })
            .or_insert_with(|| value.to_string());
    }
    out
}

fn grpc_denied_response(denied: DeniedHttpResponse) -> pingora::Result<ExternalAuthDecision> {
    let status = denied
        .status
        .and_then(|status| u16::try_from(status.code).ok())
        .filter(|status| (100..=599).contains(status))
        .unwrap_or(403);
    let body = Bytes::from(denied.body);
    let mut header = ResponseHeader::build(status, None)?;
    header.insert_header(CONTENT_LENGTH, body.len().to_string())?;
    for header_value in denied.headers {
        let Some(header_value) = header_value.header else {
            continue;
        };
        if header_value.key.eq_ignore_ascii_case("content-length") {
            continue;
        }
        if HeaderName::from_bytes(header_value.key.as_bytes()).is_err() {
            continue;
        }
        header.append_header(header_value.key, header_value.value)?;
    }
    Ok(ExternalAuthDecision::Deny(Box::new(header), body))
}

fn write_request_header_values(
    out: &mut String,
    request: &RequestHeader,
    header_name: &HeaderName,
) {
    for value in request.headers.get_all(header_name) {
        let Ok(value) = value.to_str() else {
            continue;
        };
        if value.contains('\r') || value.contains('\n') {
            continue;
        }
        out.push_str(header_name.as_str());
        out.push_str(": ");
        out.push_str(value);
        out.push_str("\r\n");
    }
}

pub(crate) fn apply_external_auth_response_headers(
    request: &mut RequestHeader,
    headers: &BTreeMap<String, Vec<String>>,
) -> pingora::Result<()> {
    for (name, values) in headers {
        if is_forbidden_backend_request_header(name) {
            continue;
        }
        let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let mut values = values.iter();
        let Some(first) = values.next() else {
            continue;
        };
        request.insert_header(header_name.as_str().to_string(), first.clone())?;
        for value in values {
            request.append_header(header_name.as_str().to_string(), value.clone())?;
        }
    }
    Ok(())
}

fn allowed_response_headers(
    headers: &[(String, String)],
    allowed: &[String],
) -> BTreeMap<String, Vec<String>> {
    let allowed: BTreeSet<String> = allowed
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let mut out = BTreeMap::new();
    for (name, value) in headers {
        let normalized = name.to_ascii_lowercase();
        if !allowed.contains(&normalized) || is_forbidden_backend_request_header(&normalized) {
            continue;
        }
        out.entry(name.clone())
            .or_insert_with(Vec::new)
            .push(value.clone());
    }
    out
}

fn is_forbidden_backend_request_header(name: &str) -> bool {
    name.eq_ignore_ascii_case("host")
        || name.eq_ignore_ascii_case("authority")
        || name.eq_ignore_ascii_case(":authority")
}

struct AuthResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: BytesMut,
}

async fn read_auth_response(stream: &mut TcpStream) -> pingora::Result<AuthResponse> {
    let mut raw = BytesMut::new();
    loop {
        let mut byte = [0u8; 1];
        let read = stream.read(&mut byte).await.map_err(external_auth_error)?;
        if read == 0 {
            return Err(pingora::Error::new(pingora::ErrorType::HTTPStatus(502)));
        }
        raw.extend_from_slice(&byte[..read]);
        if raw.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let headers = str::from_utf8(&raw)
        .map_err(|_| pingora::Error::new(pingora::ErrorType::HTTPStatus(502)))?;
    let mut lines = headers.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|status| status.parse::<u16>().ok())
        .ok_or_else(|| pingora::Error::new(pingora::ErrorType::HTTPStatus(502)))?;
    let mut response_headers = Vec::with_capacity(lines.clone().count());
    let mut content_length = 0;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse::<usize>().unwrap_or(0);
        }
        response_headers.push((name.to_string(), value.to_string()));
    }
    let mut body = BytesMut::with_capacity(content_length);
    if content_length > 0 {
        body.resize(content_length, 0);
        stream
            .read_exact(&mut body)
            .await
            .map_err(external_auth_error)?;
    }
    Ok(AuthResponse {
        status,
        headers: response_headers,
        body,
    })
}

fn external_auth_error(error: std::io::Error) -> Box<pingora::Error> {
    pingora::Error::because(
        pingora::ErrorType::HTTPStatus(502),
        "external auth HTTP request failed",
        error,
    )
}

fn grpc_external_auth_transport_error(error: tonic::transport::Error) -> Box<pingora::Error> {
    pingora::Error::because(
        pingora::ErrorType::HTTPStatus(500),
        "external auth gRPC transport failed",
        error,
    )
}

fn grpc_external_auth_status_error(error: tonic::Status) -> Box<pingora::Error> {
    pingora::Error::because(
        pingora::ErrorType::HTTPStatus(500),
        "external auth gRPC check failed",
        error,
    )
}

fn grpc_external_auth_protocol_error(message: impl Into<String>) -> Box<pingora::Error> {
    pingora::Error::explain(pingora::ErrorType::HTTPStatus(500), message.into())
}
