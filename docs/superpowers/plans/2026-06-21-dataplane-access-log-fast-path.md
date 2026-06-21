# Dataplane Access Log Fast Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore HTTP fast path eligibility when access logging is enabled while preserving route annotation overrides and access log field capture.

**Architecture:** Carry route annotations in the compiled HTTP fast path using `Arc<BTreeMap<String, String>>`, make fast selected backends visible to access log annotation resolution, and remove `access_log_enabled` from the core fast-path safety gate. The request filter will cache access log fields from lightweight Pingora request/session sources after a fast selection is accepted.

**Tech Stack:** Rust 2024, Pingora `RequestHeader` and `Session`, `ntgw-ir` HTTP fast path plan, `ntgw-http` proxy context and access log helpers, `ntgw-observability` access log resolver.

---

## Files

- Modify: `crates/ntgw-ir/src/http_fast_path.rs`
- Modify: `crates/ntgw-http/src/proxy.rs`
- Modify: `crates/ntgw-http/src/proxy/context.rs`
- Modify: `crates/ntgw-http/src/proxy/request.rs`
- Modify: `crates/ntgw-http/src/proxy/filters.rs`
- Modify: `crates/ntgw-http/src/proxy/tests/fast_path.rs`
- Modify: `crates/ntgw-http/src/proxy/logging.rs`

## Task 1: Add failing tests for access-log-enabled fast path

- [ ] Add a new unit test in `crates/ntgw-http/src/proxy/tests/fast_path.rs` after `initial_request_state_carries_fast_path_selection_from_current_snapshot`:

```rust
#[test]
fn initial_request_state_keeps_fast_path_selection_when_access_log_is_enabled() {
    let mut snapshot = sample_fast_path_snapshot();
    snapshot.rebuild_runtime_indexes();
    let cache = SelectedBackendConfigCache;
    let mut request = RequestHeader::build("GET", b"/orders?id=123", None).expect("request header");
    request.insert_header("host", "example.com").expect("host");
    let mut ctx = RequestContext::default();

    let state = prepare_initial_request_state(
        &snapshot, &cache, &mut ctx, &request, 80, Some("192.0.2.10".to_string()), None, false,
        true, 0, 0,
    )
    .expect("initial request state");

    assert!(state.fast_path_selected.is_some());
    assert_eq!(ctx.client_ip, "192.0.2.10");
    assert_eq!(ctx.host, "example.com");
    assert_eq!(ctx.path, "/orders");
    assert_eq!(ctx.request_id, "-");
    assert_eq!(ctx.snapshot_version, "snapshot-1");
}
```

- [ ] Update `fast_path_is_allowed_only_when_request_features_are_disabled` in `crates/ntgw-http/src/proxy/tests/fast_path.rs` so access logging no longer disables fast path:

```rust
#[test]
fn fast_path_is_allowed_only_when_request_features_are_disabled() {
    assert!(fast_path_request_features_are_safe(false, false, false));
    assert!(!fast_path_request_features_are_safe(true, false, false));
    assert!(!fast_path_request_features_are_safe(false, true, false));
    assert!(!fast_path_request_features_are_safe(false, false, true));
}
```

- [ ] Run the focused tests and verify RED:

```bash
cargo test -p ntgw-http initial_request_state_keeps_fast_path_selection_when_access_log_is_enabled -- --nocapture
```

Expected result: the new test fails because `state.fast_path_selected` is `None`.

```bash
cargo test -p ntgw-http fast_path_is_allowed_only_when_request_features_are_disabled -- --nocapture
```

Expected result: this test fails to compile or fails assertions until the function signature and behavior are updated.

## Task 2: Carry route annotations through compiled fast selection

- [ ] Add `BTreeMap` to the `std` imports in `crates/ntgw-ir/src/http_fast_path.rs`:

```rust
use std::{collections::BTreeMap, sync::Arc};
```

- [ ] Add route annotations to `CompiledSelectedHttpBackend`:

```rust
pub route_annotations: Arc<BTreeMap<String, String>>,
```

- [ ] Add route annotations to `CompiledHttpFastRoute`:

```rust
route_annotations: Arc<BTreeMap<String, String>>,
```

- [ ] In `HttpFastPathPlan::build`, initialize the compiled route annotations once per route:

```rust
(!eligible_rules.is_empty()).then_some(CompiledHttpFastRoute {
    route_index,
    route_annotations: Arc::new(route.annotations.clone()),
    eligible_rules,
})
```

- [ ] In `HttpFastPathPlan::select`, set the selected backend annotations:

```rust
route_annotations: Arc::clone(&compiled_route.route_annotations),
```

- [ ] Update manual `CompiledSelectedHttpBackend` test fixtures in `crates/ntgw-http/src/proxy/tests/fast_path.rs` to include:

```rust
route_annotations: Default::default(),
```

- [ ] Run the focused tests and verify GREEN for compile and existing fast path behavior:

```bash
cargo test -p ntgw-http fast_path -- --nocapture
```

Expected result: tests compile and pass after later tasks are complete; at this point failures related to the access-log gate are still expected.

## Task 3: Resolve access log annotations from fast selected backend

- [ ] Update `access_log_route_annotations` in `crates/ntgw-http/src/proxy/request.rs`:

```rust
pub(crate) fn access_log_route_annotations(ctx: &RequestContext) -> &BTreeMap<String, String> {
    if let Some(selected) = ctx.selected_backend.as_ref() {
        return &selected.route_annotations;
    }

    if let Some(selected) = ctx
        .fast_selected_backend
        .as_ref()
        .map(|state| &state.selected)
    {
        return &selected.route_annotations;
    }

    &ctx.route_annotations
}
```

- [ ] Add a unit test in `crates/ntgw-http/src/proxy/logging.rs` after `route_annotations_for_log_prefers_selected_backend_annotations`:

```rust
#[test]
fn route_annotations_for_log_prefers_fast_selected_backend_annotations() {
    let selected = CompiledSelectedHttpBackend {
        route_kind: RouteKind::Http,
        route_name: "route".to_string(),
        route_namespace: "default".to_string(),
        rule_index: None,
        route_annotations: Arc::new(BTreeMap::from([(
            "gateway.nantian.dev/access-log-sample-rate".to_string(),
            "0".to_string(),
        )])),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "HTTP".to_string(),
        backend: BackendEndpoint {
            address: "127.0.0.1".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/echo:8080".to_string(),
        matched_http_path: ntgw_ir::MatchedHttpPath::default(),
        runtime_ids: SelectedBackendRuntimeIds::default(),
    };
    let mut ctx = RequestContext {
        route_annotations: BTreeMap::from([("stale".to_string(), "1".to_string())]),
        ..RequestContext::default()
    };
    cache_fast_selected_backend_state(&mut ctx, selected, true);

    let annotations = access_log_route_annotations(&ctx);
    assert_eq!(
        annotations
            .get("gateway.nantian.dev/access-log-sample-rate")
            .map(String::as_str),
        Some("0")
    );
    assert!(!annotations.contains_key("stale"));
}
```

- [ ] Ensure the test module imports `CompiledSelectedHttpBackend` and `SelectedBackendRuntimeIds` from `ntgw_ir` if they are not already imported.

- [ ] Run and verify the focused annotation tests:

```bash
cargo test -p ntgw-http route_annotations_for_log -- --nocapture
```

Expected result: all route annotation tests pass.

## Task 4: Remove access logging from the fast path safety gate

- [ ] Change `fast_path_request_features_are_safe` in `crates/ntgw-http/src/proxy.rs` to:

```rust
pub(crate) fn fast_path_request_features_are_safe(
    request_tracing_enabled: bool,
    request_headers_required: bool,
    request_source_ip_required: bool,
) -> bool {
    !request_tracing_enabled && !request_headers_required && !request_source_ip_required
}
```

- [ ] Update the call in `prepare_initial_request_state`:

```rust
fast_path_request_features_are_safe(
    request_tracing_enabled,
    current.request_materialization.requires_full_headers(),
    current.request_materialization.source_ip,
)
```

- [ ] Keep `cache_snapshot_version_if_observed(ctx, current.id.as_str(), access_log_enabled, request_tracing_enabled)` unchanged so access-log-enabled fast-path requests keep the snapshot version.

- [ ] Run and verify the focused fast path tests:

```bash
cargo test -p ntgw-http fast_path -- --nocapture
```

Expected result: the new access-log-enabled initial request state test passes.

## Task 5: Cache fast-path access log request and connection fields

- [ ] In the fast path branch of `do_request_filter` in `crates/ntgw-http/src/proxy/filters.rs`, after `cache_fast_selected_backend_state(...)` and before assigning `ctx.selected_backend_config`, add:

```rust
let route_access_log_annotations = super::request::access_log_route_annotations(ctx).clone();
super::request::cache_access_log_connection_fields_if_needed(
    session,
    ctx,
    &proxy.access_log,
    &route_access_log_annotations,
);
super::request::cache_access_log_request_headers_from_header_if_needed(
    ctx,
    session.req_header(),
    &proxy.access_log,
    &route_access_log_annotations,
);
```

- [ ] Add a unit test in `crates/ntgw-http/src/proxy/tests/fast_path.rs` near the cache state tests:

```rust
#[test]
fn cache_fast_selected_backend_state_keeps_route_annotations_for_access_log() {
    let selected = CompiledSelectedHttpBackend {
        route_kind: RouteKind::Http,
        route_name: "orders".to_string(),
        route_namespace: "default".to_string(),
        rule_index: Some(0),
        route_annotations: Arc::new(std::collections::BTreeMap::from([(
            "gateway.nantian.dev/access-log-mode".to_string(),
            "text".to_string(),
        )])),
        listener_name: "default/gw/http".to_string(),
        listener_protocol: "HTTP".to_string(),
        backend: BackendEndpoint {
            address: "10.0.0.10".to_string(),
            port: 8080,
            healthy: true,
        },
        backend_name: "default/orders:8080".to_string(),
        matched_http_path: ntgw_ir::MatchedHttpPath::default(),
        runtime_ids: SelectedBackendRuntimeIds::default(),
    };
    let mut ctx = RequestContext {
        route_annotations: std::collections::BTreeMap::from([("stale".to_string(), "1".to_string())]),
        ..RequestContext::default()
    };

    cache_fast_selected_backend_state(&mut ctx, selected, true);

    let annotations = super::super::request::access_log_route_annotations(&ctx);
    assert_eq!(
        annotations
            .get("gateway.nantian.dev/access-log-mode")
            .map(String::as_str),
        Some("text")
    );
    assert!(!annotations.contains_key("stale"));
}
```

- [ ] Run and verify focused tests:

```bash
cargo test -p ntgw-http cache_fast_selected_backend_state -- --nocapture
```

Expected result: fast selected state tests pass.

## Task 6: Full verification and acceptance record

- [ ] Format the workspace:

```bash
cargo fmt --all
```

- [ ] Verify formatting:

```bash
cargo fmt --all -- --check
```

Expected result: exit code 0.

- [ ] Run focused HTTP and observability tests:

```bash
cargo test -p ntgw-http fast_path -- --nocapture
cargo test -p ntgw-http route_annotations_for_log -- --nocapture
cargo test -p ntgw-observability access -- --nocapture
```

Expected result: exit code 0 for all commands.

- [ ] Run affected crate tests:

```bash
cargo test -p ntgw-http -p ntgw-observability -p ntgw-bench
```

Expected result: exit code 0.

- [ ] Run affected crate clippy:

```bash
cargo clippy -p ntgw-http -p ntgw-ir -p ntgw-observability -p ntgw-bench -- -D warnings
```

Expected result: exit code 0.

- [ ] Run whitespace check:

```bash
git diff --check
```

Expected result: exit code 0.

- [ ] Run benchmark command:

```bash
cargo run -p ntgw-bench --release --features allocator-jemalloc -- --iterations 100 --output /tmp/ntgw-bench-p99-access-log-fast-path.json
```

Expected result: exit code 0 and output file exists.

- [ ] Verify repository isolation:

```bash
git -C /root/nantian-gw/gateway status --short
git -C /root/nantian-gw/dashboard status --short
git -C /root/nantian-gw/website status --short
git -C /root/nantian-gw/proto status --short
git -C /root/nantian-gw/helm-charts status --short
```

Expected result: no new changes caused by this dataplane task. Pre-existing unrelated changes may remain.

## Acceptance Results

- Baseline before implementation: `cargo test -p ntgw-http fast_path -- --nocapture` exited 0 with `11 passed; 0 failed`.
- RED verification after tests were added:
  - `cargo test -p ntgw-http fast_path_is_allowed_only_when_request_features_are_disabled -- --nocapture` exited 101 with `E0061` because `fast_path_request_features_are_safe` still accepted 4 arguments.
  - `cargo test -p ntgw-http initial_request_state_keeps_fast_path_selection_when_access_log_is_enabled -- --nocapture` exited 101 for the same compile failure before production code was changed.
- Focused GREEN verification:
  - `cargo test -p ntgw-http fast_path -- --nocapture` exited 0 with `13 passed; 0 failed`.
  - `cargo test -p ntgw-http route_annotations_for_log -- --nocapture` exited 0 with `3 passed; 0 failed`.
  - `cargo test -p ntgw-observability access -- --nocapture` exited 0 with `30 passed; 0 failed`.
- Affected crate tests: `cargo test -p ntgw-http -p ntgw-observability -p ntgw-bench` exited 0.
  - `ntgw-bench`: `10 passed; 0 failed`.
  - `ntgw-http`: `327 passed; 0 failed`.
  - `ntgw-observability`: `85 passed; 0 failed`.
  - Doc-tests for `ntgw-http` and `ntgw-observability`: `0 passed; 0 failed`.
- Lint: `cargo clippy -p ntgw-http -p ntgw-ir -p ntgw-observability -p ntgw-bench -- -D warnings` exited 0.
- Workspace test: `cargo test --workspace` exited 0. The output included passing tests for all workspace crates, including `ntgw-ai`, `ntgw-app`, `ntgw-http`, `ntgw-ir`, `ntgw-observability`, `ntgw-shared-tls`, `ntgw-stream`, `ntgw-wasm`, and `ntgw-xds`; doctests exited 0 with one ignored `ntgw-wasm-sdk` doctest.
- Workspace lint: `cargo clippy --workspace -- -D warnings` exited 0.
- Formatting: `cargo fmt --all` exited 0, then `cargo fmt --all -- --check` exited 0.
- Whitespace: `git diff --check` exited 0.
- Benchmark: `cargo run -p ntgw-bench --release --features allocator-jemalloc -- --iterations 100 --output /tmp/ntgw-bench-p99-access-log-fast-path.json` exited 0 and wrote `/tmp/ntgw-bench-p99-access-log-fast-path.json`.
  - `request_fast_path_selection`: `iterations=100`, `p50_ms=0.000392`, `p95_ms=0.000515`, `p99_ms=0.001825`, `max_ms=0.003986`.
  - `access_log_disabled_path`: `iterations=100`, `p50_ms=0.000930`, `p95_ms=0.001322`, `p99_ms=0.001709`, `max_ms=0.008552`.
  - `access_log_sampled_out_path`: `iterations=100`, `p50_ms=0.000980`, `p95_ms=0.001195`, `p99_ms=0.001414`, `max_ms=0.001543`.
  - `access_log_write_path`: `iterations=100`, `p50_ms=0.012663`, `p95_ms=0.014710`, `p99_ms=0.023190`, `max_ms=0.510151`.
- Repository isolation:
  - Dataplane worktree contains only this task's modified Rust files and `docs/superpowers/`.
  - `gateway` has pre-existing untracked files: `deploy/kubernetes/overlays/kind-pprof/`, `docs/superpowers/plans/2026-06-19-gateway-optimization-audit.md`, `docs/superpowers/plans/2026-06-19-gateway-optimization-plan.md`, and `translator.test`.
  - `website` has pre-existing generated `.astro/*` modifications.
  - `dashboard`, `proto`, and `helm-charts` reported no changes.
