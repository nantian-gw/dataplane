use super::context::remember_transport_retry_excluded_endpoint;
use super::*;
use ntgw_observability::RetryBudgetController;

pub(crate) fn retry_backoff(ctx: &RequestContext) -> Option<std::time::Duration> {
    (ctx.retry_attempts > 0)
        .then_some(ctx.retry_backoff)
        .flatten()
}

pub(crate) fn response_is_retryable(ctx: &RequestContext, status: u16) -> bool {
    selected_retry_policy(ctx).is_some_and(|policy| {
        policy.codes.contains(&(status as u32)) && request_is_retry_replayable(ctx)
    })
}

pub(crate) fn retry_completed_successfully(ctx: &RequestContext) -> bool {
    ctx.retry_attempts > 0 && matches!(ctx.status, 100..=499)
}

pub(crate) fn try_prepare_retry(
    ctx: &mut RequestContext,
    session: &Session,
    retry_budget: &RetryBudgetController,
) -> bool {
    let Some(policy) = selected_retry_policy(ctx) else {
        return false;
    };
    try_prepare_retry_with_limit(
        ctx,
        session,
        retry_budget,
        retry_limit(policy),
        policy.backoff,
        false,
        false,
    )
}

pub(crate) fn try_prepare_transport_retry(
    ctx: &mut RequestContext,
    session: &Session,
    retry_budget: &RetryBudgetController,
    error: &Error,
) -> bool {
    if let Some(policy) = selected_retry_policy(ctx) {
        let retry_limit = retry_limit(policy);
        let retry_backoff = policy.backoff;
        return try_prepare_retry_with_limit(
            ctx,
            session,
            retry_budget,
            retry_limit,
            retry_backoff,
            false,
            true,
        );
    }
    if !default_transport_retryable_error(error) {
        return false;
    }
    try_prepare_retry_with_limit(
        ctx,
        session,
        retry_budget,
        DEFAULT_TRANSPORT_CONNECT_RETRIES,
        None,
        true,
        true,
    )
}

fn try_prepare_retry_with_limit(
    ctx: &mut RequestContext,
    session: &Session,
    retry_budget: &RetryBudgetController,
    retry_limit: u32,
    retry_backoff: Option<std::time::Duration>,
    seed_retry_budget: bool,
    remember_transport_failure: bool,
) -> bool {
    if !request_is_retry_replayable(ctx) {
        return false;
    }
    if session.as_ref().retry_buffer_truncated() {
        return false;
    }
    if ctx.retry_attempts >= retry_limit {
        return false;
    }
    if seed_retry_budget && !ctx.retry_budget_seeded {
        retry_budget.observe_retryable_request();
        ctx.retry_budget_seeded = true;
    }
    if !retry_budget.try_acquire_retry() {
        return false;
    }

    if remember_transport_failure {
        remember_transport_retry_excluded_endpoint(ctx);
    }
    ctx.retry_attempts += 1;
    ctx.retry_backoff = retry_backoff;
    ctx.backend.clear();
    ctx.selected_backend = None;
    ctx.fast_selected_backend = None;
    ctx.selected_backend_config = None;
    ctx.circuit_breaker_permit = None;
    ctx.upstream_connect_started_at = None;
    true
}

fn default_transport_retryable_error(error: &Error) -> bool {
    matches!(
        error.etype(),
        ErrorType::ConnectRefused | ErrorType::ConnectNoRoute | ErrorType::ConnectTimedout
    )
}

pub(crate) fn request_is_retry_replayable(ctx: &RequestContext) -> bool {
    const REPLAYABLE_METHODS: [&str; 6] = ["GET", "HEAD", "OPTIONS", "TRACE", "PUT", "DELETE"];

    REPLAYABLE_METHODS
        .iter()
        .any(|method| ctx.method.eq_ignore_ascii_case(method))
}

pub(crate) fn selected_retry_policy(ctx: &RequestContext) -> Option<&RetryPolicy> {
    ctx.selected_backend
        .as_ref()
        .and_then(|selected| selected.retry.as_ref())
}

pub(crate) fn retry_limit(policy: &RetryPolicy) -> u32 {
    if policy.attempts == 0 {
        DEFAULT_HTTP_ROUTE_RETRIES
    } else {
        policy.attempts
    }
}

pub(crate) fn retry_status_error(status: u16, retry: bool) -> Box<Error> {
    let mut error = Error::new(ErrorType::HTTPStatus(502));
    error.set_retry(retry);
    error.more_context(format!(
        "configured HTTPRoute retry policy intercepted upstream status {status}"
    ))
}

pub(crate) fn proxy_error_status(error_type: &str) -> u16 {
    match error_type {
        "NoRouteMatched" => 404,
        "NoHealthyBackend" => 503,
        "CircuitBreakerOpen" => 503,
        "UnsupportedRouteFilter" => 500,
        item if item.ends_with("Timedout") || item.ends_with("Timeout") => 504,
        _ => 500,
    }
}

pub(crate) fn proxy_error_flag(error_type: &str) -> &'static str {
    match error_type {
        "NoRouteMatched" => "NR",
        "NoHealthyBackend" => "UH",
        "CircuitBreakerOpen" => "CB",
        "InvalidBackendRefs" => "IB",
        "UnsupportedRouteFilter" => "UF",
        item if is_timeout_error_name(item) => "UT",
        _ => "UF",
    }
}

pub(crate) fn proxy_error_flag_for(error: &Error) -> &'static str {
    match (error.esource(), error.etype()) {
        (ErrorSource::Downstream, ErrorType::ConnectionClosed) => "DC",
        (ErrorSource::Downstream, item) if is_timeout_error_name(item.as_str()) => "IT",
        (ErrorSource::Upstream, ErrorType::ConnectionClosed) => "UC",
        _ => proxy_error_flag(error.etype().as_str()),
    }
}

pub(crate) fn proxy_error_code(error: &Error) -> u16 {
    match (error.esource(), error.etype()) {
        (ErrorSource::Downstream, ErrorType::ConnectionClosed) => 499,
        (ErrorSource::Downstream, item) if is_timeout_error_name(item.as_str()) => 408,
        (_, ErrorType::HTTPStatus(code)) => *code,
        (_, ErrorType::CustomCode(_, code)) => *code,
        _ => proxy_error_status(error.etype().as_str()),
    }
}

fn is_timeout_error_name(value: &str) -> bool {
    value.ends_with("Timedout") || value.ends_with("Timeout")
}

pub(crate) fn is_downstream_connection_closed(error: &Error) -> bool {
    matches!(
        (error.esource(), error.etype()),
        (ErrorSource::Downstream, ErrorType::ConnectionClosed)
    )
}

pub(crate) fn should_suppress_proxy_error_log(error: &Error, response_started: bool) -> bool {
    is_downstream_connection_closed(error)
        || error.etype().as_str() == "NoRouteMatched"
        || (response_started
            && matches!(
                (error.esource(), error.etype()),
                (ErrorSource::Upstream, ErrorType::ConnectionClosed)
            ))
}
