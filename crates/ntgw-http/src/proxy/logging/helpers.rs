use std::borrow::Cow;

use super::super::context::RequestContext;

pub(crate) fn extract_request_header<'a>(ctx: &'a RequestContext, name: &str) -> Cow<'a, str> {
    ctx.access_log_request_headers
        .get(name)
        .map(|s| s.as_str())
        .or_else(|| {
            ctx.request_headers
                .as_ref()
                .and_then(|headers| headers.get(name))
                .and_then(|values| values.first())
                .map(|s| s.as_str())
        })
        .filter(|s| !s.is_empty())
        .map(Cow::Borrowed)
        .unwrap_or(Cow::Borrowed("-"))
}

pub(crate) fn build_request_line(ctx: &RequestContext) -> String {
    let path_and_query = if ctx.query_string.is_empty() {
        ctx.path.clone()
    } else {
        format!("{}?{}", ctx.path, ctx.query_string)
    };
    let version = if ctx.http_version.is_empty() {
        String::from("HTTP/1.1")
    } else {
        ctx.http_version.clone()
    };
    format!("{} {} {}", ctx.method, path_and_query, version)
}
