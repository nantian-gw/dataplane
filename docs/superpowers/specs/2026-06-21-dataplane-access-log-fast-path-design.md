# Dataplane Access Log Fast Path Design

## Problem

Production dataplane configuration enables HTTP access logging with a low sample rate. The current HTTP fast path gate rejects every request when `accessLog.enabled` is true, even when the request will not emit a sampled access log line and even when access logging only needs fields that are already available from `RequestHeader` and `RequestContext`.

This keeps otherwise simple HTTP routes on the slower selection path and can raise p99 latency on high-cardinality or high-throughput traffic.

## Goals

- Allow HTTP fast path selection while `accessLog.enabled` is true when tracing is disabled and the snapshot does not require full request headers or source IP for routing.
- Preserve route-level access log annotation semantics for fast-path requests, including `enabled`, `format`, `mode`, and `sample-rate` overrides.
- Keep the access log sampling and rendering path shared with normal selection by using `resolve_access_log_write_options` and `access_log_route_annotations`.
- Capture access log request headers, response headers, upstream status, scheme, and remote port from existing lightweight sources when a fast-path route needs them.
- Avoid introducing unsafe code or changing protobuf, Helm, gateway, dashboard, website, or root workspace behavior.

## Non-Goals

- Do not change the existing behavior where a globally disabled `accessLog.enabled` short-circuits request context capture and final access log emission.
- Do not broaden HTTP fast path eligibility for routes with filters, retry, timeout, session persistence, header matches, query matches, gRPC requests, backend TLS listeners, or non-fast-path backend policies.
- Do not change access log format parsing, sampling hash inputs, writer behavior, or traffic statistics semantics.

## Design

Fast path selection should no longer treat access logging itself as unsafe. The remaining safety gate is request tracing, snapshot full-header materialization, and snapshot source-IP materialization. These features still require the normal path because they either mutate upstream headers or depend on data not represented by the fast path selection.

The compiled HTTP fast path will carry route annotations alongside route identity. To avoid per-request deep copies of route annotations, `CompiledHttpFastRoute` will store `Arc<BTreeMap<String, String>>`, created once when the fast path plan is built. `CompiledSelectedHttpBackend` will carry an `Arc` clone of that map.

`access_log_route_annotations(ctx)` will prefer annotations from `ctx.selected_backend`, then from `ctx.fast_selected_backend`, then from `ctx.route_annotations`. This mirrors normal selected-backend behavior and prevents stale pre-selection annotations from affecting fast-path access logs.

When the request filter accepts a fast path selection, it will cache the fast selected backend, then:

- cache access log connection fields from the Pingora session when the selected route's resolved log format requires `$scheme` or `$remote_port`;
- cache required request headers directly from `session.req_header()` when the selected route's resolved log format references `$http_*` variables;
- continue relying on the existing response filter to cache sent response headers, upstream response headers, and upstream status because those code paths already call `access_log_route_annotations(ctx)`.

This keeps the fast path free of full `RequestMeta` materialization while preserving the normal access log output for eligible routes.

## Expected Behavior

- A default HTTP route with access logging enabled should still produce an initial fast path selection.
- A route annotation such as `gateway.nantian.dev/access-log-mode=json` or `gateway.nantian.dev/access-log-sample-rate=0.0` should be visible to final access log resolution on the fast path.
- A text access log format that references `$http_user_agent`, `$scheme`, or `$remote_port` should use the same lightweight capture helpers on the fast path as on the normal path.
- Existing traffic metrics and selected backend config caching should remain unchanged.

## Acceptance Criteria

- In the `dataplane` worktree, `cargo test -p ntgw-http fast_path -- --nocapture` passes.
- In the `dataplane` worktree, `cargo test -p ntgw-http route_annotations_for_log -- --nocapture` passes.
- In the `dataplane` worktree, `cargo test -p ntgw-observability access -- --nocapture` passes.
- In the `dataplane` worktree, `cargo test -p ntgw-http -p ntgw-observability -p ntgw-bench` passes.
- In the `dataplane` worktree, `cargo clippy -p ntgw-http -p ntgw-ir -p ntgw-observability -p ntgw-bench -- -D warnings` passes.
- In the `dataplane` worktree, `cargo fmt --all -- --check` passes.
- In the `dataplane` worktree, `git diff --check` passes.
- A release benchmark command is run from the `dataplane` worktree and writes `/tmp/ntgw-bench-p99-access-log-fast-path.json`.
- `git status --short` for sibling repositories `gateway`, `dashboard`, `website`, `proto`, and `helm-charts` is checked and no unrelated repository is modified by this task.
