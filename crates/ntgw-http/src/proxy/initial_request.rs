use super::*;

pub(crate) fn route_filters_have_request_mirror(filters: &[Filter]) -> bool {
    filters.iter().any(|filter| filter.request_mirror.is_some())
}

pub(crate) fn unmatched_traffic_topology(listener_name: &str) -> Arc<TrafficTopology> {
    Arc::new(TrafficTopology::unmatched(listener_name))
}

pub(crate) fn fast_path_request_features_are_safe(
    request_tracing_enabled: bool,
    request_headers_required: bool,
    request_source_ip_required: bool,
) -> bool {
    !request_tracing_enabled && !request_headers_required && !request_source_ip_required
}

#[allow(private_interfaces)]
pub(crate) struct InitialFastPathSelection {
    pub(super) selected: ntgw_ir::CompiledSelectedHttpBackend,
    pub(super) config: Arc<SelectedBackendConfig>,
    pub(super) frontend_client_certificate_requirement: FrontendClientCertificateRequirement,
}

#[allow(private_interfaces)]
pub(crate) struct InitialRequestState {
    pub(super) request_header_bytes: usize,
    pub(super) misdirected_request: bool,
    pub(super) request_source_ip: Option<String>,
    pub(super) fast_path_selected: Option<InitialFastPathSelection>,
}

#[allow(clippy::too_many_arguments)]
#[allow(private_interfaces)]
pub(crate) fn prepare_initial_request_state(
    current: &Snapshot,
    selected_backend_config_cache: &SelectedBackendConfigCache,
    ctx: &mut RequestContext,
    request_header: &RequestHeader,
    request_server_port: u32,
    request_source_ip: Option<String>,
    downstream_tls_server_name: Option<&str>,
    request_tracing_enabled: bool,
    access_log_enabled: bool,
    max_request_body_bytes: usize,
    max_request_header_bytes: usize,
) -> pingora::Result<InitialRequestState> {
    let request_context_needs_source_ip = access_log_enabled || request_tracing_enabled;
    let request_context_needs_observability_fields = access_log_enabled || request_tracing_enabled;
    let request_view = RequestView::from_header_with_port(request_header, request_server_port);
    capture_request_context_from_view_for_limits(
        ctx,
        &request_view,
        request_context_needs_source_ip
            .then_some(request_source_ip.as_deref())
            .flatten(),
        request_context_needs_observability_fields,
        max_request_body_bytes > 0,
    );
    start_request_span_from_header_if_enabled(ctx, request_header, request_tracing_enabled);
    let request_header_bytes =
        request_header_bytes_for_limit(&request_view, max_request_header_bytes);
    let misdirected_request = https_request_is_misdirected_in_snapshot(
        current,
        &request_view,
        downstream_tls_server_name,
    );

    let fast_path_selected = if !misdirected_request
        && fast_path_request_features_are_safe(
            request_tracing_enabled,
            current.request_materialization.requires_full_headers(),
            current.request_materialization.source_ip,
        ) {
        cache_snapshot_version_if_observed(
            ctx,
            current.id.as_str(),
            access_log_enabled,
            request_tracing_enabled,
        );
        record_request_span(ctx);
        current
            .select_http_fast_path(fast_path_request_from_header(
                request_header,
                request_server_port,
            ))
            .map(|selected| {
                let config = selected_backend_config_cached_for_fast_path(
                    selected_backend_config_cache,
                    current,
                    &selected,
                )?;
                let frontend_client_certificate_requirement = current
                    .frontend_client_certificate_requirement(selected.listener_name.as_ref());
                Ok::<_, Box<Error>>(InitialFastPathSelection {
                    selected,
                    config,
                    frontend_client_certificate_requirement,
                })
            })
            .transpose()?
    } else {
        None
    };

    Ok(InitialRequestState {
        request_header_bytes,
        misdirected_request,
        request_source_ip,
        fast_path_selected,
    })
}
