use bytes::Bytes;
use pingora::http::ResponseHeader;

use ntgw_ir::{DirectResponseFilter, ExtensionFilter, Filter};

pub(crate) fn direct_response_filter(filters: &[Filter]) -> Option<&DirectResponseFilter> {
    filters.iter().find_map(|filter| {
        filter
            .extension_ref
            .as_ref()
            .filter(|item| item.resolved)
            .and_then(|item| match item.extension_type.as_str() {
                "DirectResponse" => item.direct_response.as_ref(),
                _ => None,
            })
    })
}

pub(crate) fn extension_filters_supported(filters: &[Filter]) -> bool {
    filters
        .iter()
        .filter_map(|filter| filter.extension_ref.as_ref())
        .all(extension_filter_supported)
}

pub(crate) fn build_direct_response(
    filter: &DirectResponseFilter,
) -> pingora::Result<(ResponseHeader, Option<Bytes>)> {
    let status_code = normalize_status_code(filter.status_code);
    let body = (!filter.body.is_empty()).then(|| Bytes::from(filter.body.clone()));
    let mut response = ResponseHeader::build(status_code, None)?;

    for header in &filter.headers {
        response.insert_header(header.name.clone(), header.value.clone())?;
    }

    if let Some(body) = body.as_ref() {
        if !filter.content_type.is_empty() {
            response.insert_header("content-type", filter.content_type.clone())?;
        }
        response.insert_header("content-length", body.len().to_string())?;
    } else {
        response.insert_header("content-length", "0")?;
    }

    Ok((response, body))
}

fn extension_filter_supported(filter: &ExtensionFilter) -> bool {
    filter.resolved
        && match filter.extension_type.as_str() {
            "DirectResponse" => filter.direct_response.is_some(),
            _ => false,
        }
}

fn normalize_status_code(status_code: u16) -> u16 {
    if (100..=599).contains(&status_code) {
        status_code
    } else {
        500
    }
}

#[cfg(test)]
mod tests;
