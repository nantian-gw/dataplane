use std::collections::BTreeMap;

use pingora::http::RequestHeader;

use super::extract::{
    grpc_content_type_headers, request_content_length_from_header,
    request_header_bytes_from_header, request_headers, request_host_value, request_id_from_header,
};

pub(crate) struct RequestView<'a> {
    req: &'a RequestHeader,
    port: u32,
}

impl<'a> RequestView<'a> {
    pub(crate) fn from_header_with_port(req: &'a RequestHeader, port: u32) -> Self {
        Self { req, port }
    }

    pub(crate) fn materialize(&self) -> ntgw_ir::RequestMeta {
        ntgw_ir::RequestMeta::with_port(
            self.raw_host().map(ToOwned::to_owned),
            self.port,
            self.path_and_query(),
            self.method(),
            request_headers(self.req),
        )
    }

    pub(crate) fn routing_key(&self) -> ntgw_ir::RequestMeta {
        ntgw_ir::RequestMeta::with_port(
            self.raw_host().map(ToOwned::to_owned),
            self.port,
            self.path_and_query(),
            self.method(),
            BTreeMap::new(),
        )
    }

    pub(crate) fn selection_meta(&self, materialize_headers: bool) -> ntgw_ir::RequestMeta {
        if materialize_headers {
            return self.materialize();
        }

        ntgw_ir::RequestMeta::with_port(
            self.raw_host().map(ToOwned::to_owned),
            self.port,
            self.path_and_query(),
            self.method(),
            grpc_content_type_headers(self.req),
        )
    }

    pub(crate) fn raw_host(&self) -> Option<&'a str> {
        request_host_value(self.req)
    }

    pub(crate) fn host(&self) -> Option<&'a str> {
        self.raw_host().map(normalize_authority_host_ref)
    }

    pub(crate) fn path(&self) -> &'a str {
        self.req.uri.path()
    }

    fn path_and_query(&self) -> &'a str {
        self.req
            .uri
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or_else(|| self.req.uri.path())
    }

    pub(crate) fn method(&self) -> &'a str {
        self.req.method.as_str()
    }

    pub(crate) fn request_id(&self) -> &'a str {
        request_id_from_header(self.req)
    }

    pub(crate) fn content_length(&self) -> usize {
        request_content_length_from_header(self.req)
    }

    pub(crate) fn header_bytes(&self) -> usize {
        request_header_bytes_from_header(self.req)
    }
}

pub(crate) fn request_header_bytes_for_limit(
    request: &RequestView<'_>,
    max_request_header_bytes: usize,
) -> usize {
    if max_request_header_bytes == 0 {
        0
    } else {
        request.header_bytes()
    }
}

pub(crate) fn normalize_authority_host_ref(host: &str) -> &str {
    if let Some(host) = host.strip_prefix('[') {
        return host.split_once(']').map(|(value, _)| value).unwrap_or(host);
    }
    host.split(':').next().unwrap_or(host)
}
