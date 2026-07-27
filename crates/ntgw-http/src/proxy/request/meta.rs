use http::header::CONTENT_TYPE;
use pingora::http::RequestHeader;
use pingora::prelude::Session;

use super::extract::client_ip;
use super::tracing::server_port;
use super::view::RequestView;

pub(crate) fn build_request_meta(session: &Session) -> ntgw_ir::RequestMeta {
    let req = session.req_header();
    let mut meta = build_request_meta_from_header_with_port(req, server_port(session));
    meta.source_ip = client_ip(session);
    meta
}

#[cfg(test)]
pub(crate) fn build_request_meta_from_header(req: &RequestHeader) -> ntgw_ir::RequestMeta {
    build_request_meta_from_header_with_port(req, 0)
}

pub(crate) fn build_request_meta_from_header_with_port(
    req: &RequestHeader,
    port: u32,
) -> ntgw_ir::RequestMeta {
    let mut meta = RequestView::from_header_with_port(req, port).materialize();
    meta.source_ip = None;
    meta
}

pub(crate) fn fast_path_request_from_header(
    req: &RequestHeader,
    port: u32,
) -> ntgw_ir::HttpFastPathRequest<'_> {
    let view = RequestView::from_header_with_port(req, port);
    ntgw_ir::HttpFastPathRequest {
        host: view.raw_host(),
        port,
        path: view.path(),
        method: view.method(),
        is_grpc: req
            .headers
            .get_all(CONTENT_TYPE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .any(|value| value.starts_with("application/grpc")),
    }
}

pub(crate) fn build_selection_request_meta_from_header_with_port(
    req: &RequestHeader,
    port: u32,
    source_ip: Option<String>,
    materialize_headers: bool,
) -> ntgw_ir::RequestMeta {
    let mut meta =
        RequestView::from_header_with_port(req, port).selection_meta(materialize_headers);
    meta.source_ip = source_ip;
    meta
}

pub(crate) fn build_selection_request_meta(
    session: &Session,
    source_ip: Option<String>,
    port: u32,
    materialize_headers: bool,
) -> ntgw_ir::RequestMeta {
    build_selection_request_meta_from_header_with_port(
        session.req_header(),
        port,
        source_ip,
        materialize_headers,
    )
}
