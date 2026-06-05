use super::*;

impl GatewayProxy {
    pub(crate) fn seed_retry_budget(&self, ctx: &mut RequestContext) {
        if ctx.retry_budget_seeded
            || selected_retry_policy(ctx).is_none()
            || !request_is_retry_replayable(ctx)
        {
            return;
        }

        self.retry_budget.observe_retryable_request();
        ctx.retry_budget_seeded = true;
    }

    pub(crate) async fn try_admit_listener(
        &self,
        session: &mut Session,
        ctx: &mut RequestContext,
    ) -> pingora::Result<bool> {
        let Some(listener_name) = self.listener_name_hint.as_deref() else {
            return Ok(true);
        };

        match self.admission.try_acquire_listener(listener_name) {
            Ok(permit) => {
                assign_ctx_string(&mut ctx.listener_name, listener_name);
                store_admission_permit(ctx, permit);
                Ok(true)
            }
            Err(_) => {
                assign_ctx_string(&mut ctx.listener_name, listener_name);
                respond_overloaded(session, ctx).await?;
                Ok(false)
            }
        }
    }

    pub(crate) async fn try_admit_grpc_request(
        &self,
        session: &mut Session,
        ctx: &mut RequestContext,
        selected: &SelectedBackend,
    ) -> pingora::Result<bool> {
        let route_key = route_budget_key_if_enabled(
            self.admission.route_scope_enabled(),
            route_kind_name(&selected.route_kind),
            selected.route_namespace.as_str(),
            selected.route_name.as_str(),
        );
        let permit = if let Some(route_key) = route_key.as_deref() {
            if ctx.admission_permit.is_some() {
                self.admission.try_acquire_route(route_key)
            } else {
                self.admission
                    .try_acquire(selected.listener_name.as_str(), route_key)
            }
        } else if ctx.admission_permit.is_some() {
            return Ok(true);
        } else {
            self.admission
                .try_acquire_listener(selected.listener_name.as_str())
        };
        match permit {
            Ok(permit) => {
                store_admission_permit(ctx, permit);
                Ok(true)
            }
            Err(_) => {
                cache_selected_backend_ref(ctx, selected, self.access_log.enabled);
                respond_overloaded(session, ctx).await?;
                Ok(false)
            }
        }
    }

    pub(crate) async fn try_rate_limit_listener(
        &self,
        session: &mut Session,
        ctx: &mut RequestContext,
    ) -> pingora::Result<bool> {
        let Some(listener_name) = self.listener_name_hint.as_deref() else {
            return Ok(true);
        };

        match self.rate_limit.try_acquire_listener(listener_name) {
            Ok(applied) => {
                assign_ctx_string(&mut ctx.listener_name, listener_name);
                ctx.rate_limit_applied |= applied;
                Ok(true)
            }
            Err(_) => {
                assign_ctx_string(&mut ctx.listener_name, listener_name);
                respond_rate_limited(session, ctx).await?;
                Ok(false)
            }
        }
    }

    pub(crate) async fn try_admit_http_route(
        &self,
        session: &mut Session,
        ctx: &mut RequestContext,
        route: &SelectedHttpRoute,
    ) -> pingora::Result<bool> {
        let route_key = route_budget_key_if_enabled(
            self.admission.route_scope_enabled(),
            "Http",
            route.route_namespace.as_str(),
            route.route_name.as_str(),
        );
        let permit = if let Some(route_key) = route_key.as_deref() {
            if ctx.admission_permit.is_some() {
                self.admission.try_acquire_route(route_key)
            } else {
                self.admission
                    .try_acquire(route.listener_name.as_str(), route_key)
            }
        } else if ctx.admission_permit.is_some() {
            return Ok(true);
        } else {
            self.admission
                .try_acquire_listener(route.listener_name.as_str())
        };
        match permit {
            Ok(permit) => {
                store_admission_permit(ctx, permit);
                Ok(true)
            }
            Err(_) => {
                assign_ctx_string(&mut ctx.route_kind, "Http");
                assign_ctx_string(&mut ctx.route_name, route.route_name.as_str());
                assign_ctx_string(&mut ctx.route_namespace, route.route_namespace.as_str());
                cache_route_annotations(ctx, &self.access_log, &route.route_annotations);
                assign_ctx_string(&mut ctx.listener_name, route.listener_name.as_str());
                assign_ctx_string(&mut ctx.listener_protocol, route.listener_protocol.as_str());
                if let Some(backend_name) = route.backend_name.as_deref() {
                    assign_ctx_string(&mut ctx.backend, backend_name);
                }
                respond_overloaded(session, ctx).await?;
                Ok(false)
            }
        }
    }

    pub(crate) async fn try_admit_fast_http_route(
        &self,
        session: &mut Session,
        ctx: &mut RequestContext,
        selected: &aeg_ir::CompiledSelectedHttpBackend,
    ) -> pingora::Result<bool> {
        let route_key = route_budget_key_if_enabled(
            self.admission.route_scope_enabled(),
            "Http",
            selected.route_namespace.as_str(),
            selected.route_name.as_str(),
        );
        let permit = if let Some(route_key) = route_key.as_deref() {
            if ctx.admission_permit.is_some() {
                self.admission.try_acquire_route(route_key)
            } else {
                self.admission
                    .try_acquire(selected.listener_name.as_str(), route_key)
            }
        } else if ctx.admission_permit.is_some() {
            return Ok(true);
        } else {
            self.admission
                .try_acquire_listener(selected.listener_name.as_str())
        };
        match permit {
            Ok(permit) => {
                store_admission_permit(ctx, permit);
                Ok(true)
            }
            Err(_) => {
                cache_fast_selected_backend_state(
                    ctx,
                    selected.clone(),
                    self.selected_display_fields_needed(ctx),
                );
                respond_overloaded(session, ctx).await?;
                Ok(false)
            }
        }
    }

    pub(crate) async fn try_rate_limit_grpc_request(
        &self,
        session: &mut Session,
        ctx: &mut RequestContext,
        selected: &SelectedBackend,
    ) -> pingora::Result<bool> {
        let route_key = route_budget_key_if_enabled(
            self.rate_limit.route_scope_enabled(),
            route_kind_name(&selected.route_kind),
            selected.route_namespace.as_str(),
            selected.route_name.as_str(),
        );
        let limited = if let Some(route_key) = route_key.as_deref() {
            if self.listener_name_hint.is_none() {
                self.rate_limit
                    .try_acquire(selected.listener_name.as_str(), route_key)
            } else {
                self.rate_limit.try_acquire_route(route_key)
            }
        } else if self.listener_name_hint.is_none() {
            self.rate_limit
                .try_acquire_listener(selected.listener_name.as_str())
        } else {
            return Ok(true);
        };
        match limited {
            Ok(applied) => {
                ctx.rate_limit_applied |= applied;
                Ok(true)
            }
            Err(_) => {
                cache_selected_backend_ref(ctx, selected, self.access_log.enabled);
                respond_rate_limited(session, ctx).await?;
                Ok(false)
            }
        }
    }

    pub(crate) async fn try_rate_limit_http_route(
        &self,
        session: &mut Session,
        ctx: &mut RequestContext,
        route: &SelectedHttpRoute,
    ) -> pingora::Result<bool> {
        let route_key = route_budget_key_if_enabled(
            self.rate_limit.route_scope_enabled(),
            "Http",
            route.route_namespace.as_str(),
            route.route_name.as_str(),
        );
        let limited = if let Some(route_key) = route_key.as_deref() {
            if self.listener_name_hint.is_none() {
                self.rate_limit
                    .try_acquire(route.listener_name.as_str(), route_key)
            } else {
                self.rate_limit.try_acquire_route(route_key)
            }
        } else if self.listener_name_hint.is_none() {
            self.rate_limit
                .try_acquire_listener(route.listener_name.as_str())
        } else {
            return Ok(true);
        };
        match limited {
            Ok(applied) => {
                ctx.rate_limit_applied |= applied;
                Ok(true)
            }
            Err(_) => {
                assign_ctx_string(&mut ctx.route_kind, "Http");
                assign_ctx_string(&mut ctx.route_name, route.route_name.as_str());
                assign_ctx_string(&mut ctx.route_namespace, route.route_namespace.as_str());
                cache_route_annotations(ctx, &self.access_log, &route.route_annotations);
                assign_ctx_string(&mut ctx.listener_name, route.listener_name.as_str());
                assign_ctx_string(&mut ctx.listener_protocol, route.listener_protocol.as_str());
                if let Some(backend_name) = route.backend_name.as_deref() {
                    assign_ctx_string(&mut ctx.backend, backend_name);
                }
                respond_rate_limited(session, ctx).await?;
                Ok(false)
            }
        }
    }

    pub(crate) async fn try_rate_limit_fast_http_route(
        &self,
        session: &mut Session,
        ctx: &mut RequestContext,
        selected: &aeg_ir::CompiledSelectedHttpBackend,
    ) -> pingora::Result<bool> {
        let route_key = route_budget_key_if_enabled(
            self.rate_limit.route_scope_enabled(),
            "Http",
            selected.route_namespace.as_str(),
            selected.route_name.as_str(),
        );
        let limited = if let Some(route_key) = route_key.as_deref() {
            if self.listener_name_hint.is_none() {
                self.rate_limit
                    .try_acquire(selected.listener_name.as_str(), route_key)
            } else {
                self.rate_limit.try_acquire_route(route_key)
            }
        } else if self.listener_name_hint.is_none() {
            self.rate_limit
                .try_acquire_listener(selected.listener_name.as_str())
        } else {
            return Ok(true);
        };
        match limited {
            Ok(applied) => {
                ctx.rate_limit_applied |= applied;
                Ok(true)
            }
            Err(_) => {
                cache_fast_selected_backend_state(
                    ctx,
                    selected.clone(),
                    self.selected_display_fields_needed(ctx),
                );
                respond_rate_limited(session, ctx).await?;
                Ok(false)
            }
        }
    }
}

fn route_budget_key_if_enabled(
    enabled: bool,
    route_kind: &str,
    namespace: &str,
    name: &str,
) -> Option<String> {
    enabled.then(|| route_budget_key(route_kind, namespace, name))
}

async fn respond_overloaded(
    session: &mut Session,
    ctx: &mut RequestContext,
) -> pingora::Result<()> {
    ctx.status = 503;
    assign_ctx_string(&mut ctx.response_flags, "OL");
    session.respond_error(503).await
}

async fn respond_rate_limited(
    session: &mut Session,
    ctx: &mut RequestContext,
) -> pingora::Result<()> {
    ctx.status = 429;
    assign_ctx_string(&mut ctx.response_flags, "RL");
    session.respond_error(429).await
}

#[cfg(test)]
mod tests {
    use super::route_budget_key_if_enabled;

    #[test]
    fn route_budget_key_is_only_formatted_for_enabled_scope() {
        assert_eq!(
            route_budget_key_if_enabled(false, "Http", "default", "orders"),
            None
        );
        assert_eq!(
            route_budget_key_if_enabled(true, "Http", "default", "orders").as_deref(),
            Some("Http/default/orders")
        );
    }
}
