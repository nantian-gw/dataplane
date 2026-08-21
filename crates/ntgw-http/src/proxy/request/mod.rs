mod cache;
mod context;
mod extract;
mod meta;
pub(crate) mod trace;
mod tracing;
mod view;

// Re-export items used by external non-test code.
pub(crate) use self::cache::{
    access_log_response_requirements, access_log_route_annotations,
    cache_access_log_connection_fields_if_needed,
    cache_access_log_request_headers_from_header_if_needed, cache_access_log_response_headers,
    cache_access_log_sent_response_headers_if_needed,
    cache_request_headers_for_filters_and_access_log, cache_request_headers_if_needed,
};
pub(crate) use self::context::{
    capture_request_context, capture_request_context_from_view,
    capture_request_context_from_view_for_limits, response_filters_need_request_headers,
};
pub(crate) use self::extract::client_ip;
pub(crate) use self::meta::{
    build_request_meta, build_request_meta_from_header_with_port, build_request_meta_with_headers,
    build_selection_request_meta, fast_path_request_from_header,
};
pub(crate) use self::tracing::{
    inject_request_span_context, record_request_span, server_port, start_request_span_from_header,
    start_request_span_from_header_if_enabled, start_request_span_if_enabled,
};
pub(crate) use self::view::{RequestView, request_header_bytes_for_limit};

// Re-export items only used by test code.
#[cfg(test)]
pub(crate) use self::cache::{
    cache_access_log_connection_fields_from_sources_if_needed,
    cache_access_log_sent_response_headers_from_written_response_if_needed,
    cache_access_log_upstream_response_headers_if_needed,
    record_access_log_upstream_status_if_needed,
};
#[cfg(test)]
pub(crate) use self::context::{
    capture_request_context_from_view_for_features, effective_http_protocol,
};
#[cfg(test)]
pub(crate) use self::extract::{normalize_ip, request_id_from_headers};
#[cfg(test)]
pub(crate) use self::meta::build_request_meta_from_header;
#[cfg(test)]
pub(crate) use self::meta::build_selection_request_meta_from_header_with_port;
#[cfg(test)]
pub(crate) use self::tracing::start_request_span;
