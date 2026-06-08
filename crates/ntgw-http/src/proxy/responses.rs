use std::{boxed::Box, collections::BTreeMap};

use bytes::Bytes;
use http::{HeaderMap, HeaderValue};
use pingora::{http::ResponseHeader, protocols::http::HttpTask, proxy::Session};

use crate::extensions::build_direct_response;
use crate::filters::apply_response_filters;
use ntgw_ir::{Filter, RequestMeta};

pub(crate) async fn write_direct_response(
    session: &mut Session,
    filter: &ntgw_ir::DirectResponseFilter,
    filters: &[Filter],
    request_method: Option<&str>,
    request_headers: Option<&BTreeMap<String, Vec<String>>>,
) -> pingora::Result<u16> {
    let (mut response, body) = build_direct_response(filter)?;
    apply_response_filters(&mut response, filters, request_method, request_headers)?;
    let status = response.status.as_u16();
    if let Some(body) = body {
        session
            .write_response_header(Box::new(response), false)
            .await?;
        session.write_response_body(Some(body), true).await?;
    } else {
        session
            .write_response_header(Box::new(response), true)
            .await?;
    }
    Ok(status)
}

pub(crate) async fn write_http_no_route_response(session: &mut Session) -> pingora::Result<u16> {
    session
        .respond_error_with_body(404, Bytes::from_static(b"route not found"))
        .await?;
    Ok(404)
}

pub(crate) async fn write_grpc_no_route_response(session: &mut Session) -> pingora::Result<u16> {
    let mut response = ResponseHeader::build(200, None)?;
    response.insert_header("content-type", "application/grpc")?;
    let mut trailers = HeaderMap::new();
    trailers.insert("grpc-status", HeaderValue::from_static("12"));
    session
        .write_response_tasks(vec![
            HttpTask::Header(Box::new(response), false),
            HttpTask::Trailer(Some(Box::new(trailers))),
        ])
        .await?;
    Ok(200)
}

pub(crate) fn request_is_grpc(request: &RequestMeta) -> bool {
    request.headers.get("content-type").is_some_and(|values| {
        values
            .iter()
            .any(|value| value.starts_with("application/grpc"))
    })
}
