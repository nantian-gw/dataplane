use super::super::context::{RequestContext, route_kind_name};
use super::types::RequestRouteLabels;

pub(crate) fn request_route_labels(ctx: &RequestContext) -> RequestRouteLabels<'_> {
    if let Some(selected) = ctx.selected_backend.as_ref() {
        return RequestRouteLabels {
            listener_name: selected.listener_name.as_ref(),
            listener_protocol: selected.listener_protocol.as_ref(),
            route_namespace: selected.route_namespace.as_ref(),
            route_name: selected.route_name.as_ref(),
            route_kind: route_kind_name(&selected.route_kind),
            backend_name: selected.backend_name.as_ref(),
        };
    }

    if let Some(selected) = ctx
        .fast_selected_backend
        .as_ref()
        .map(|state| &state.selected)
    {
        return RequestRouteLabels {
            listener_name: selected.listener_name.as_ref(),
            listener_protocol: selected.listener_protocol.as_ref(),
            route_namespace: selected.route_namespace.as_ref(),
            route_name: selected.route_name.as_ref(),
            route_kind: route_kind_name(&selected.route_kind),
            backend_name: selected.backend_name.as_ref(),
        };
    }

    RequestRouteLabels {
        listener_name: ctx.listener_name.as_str(),
        listener_protocol: ctx.listener_protocol.as_str(),
        route_namespace: ctx.route_namespace.as_str(),
        route_name: ctx.route_name.as_str(),
        route_kind: ctx.route_kind.as_str(),
        backend_name: ctx.backend.as_str(),
    }
}
