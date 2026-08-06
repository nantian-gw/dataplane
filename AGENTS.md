# AGENTS.md — Nantian Gateway Data Plane

Rust workspace for the Nantian Gateway high-performance HTTP/stream proxy data plane.

## Build & Test

```bash
# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Lint (must pass in CI)
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check

# Build only the binary with jemalloc allocator
cargo build --release -p ntgw-app --features allocator-jemalloc
```

## Toolchain

- **Pinned to Rust 1.96.0** (`rust-toolchain.toml`)
- Required components: `rustfmt`, `clippy`
- No system `protoc` needed — `ntgw-proto` build.rs uses `protoc-bin-vendored`

## Architecture

This is a **monorepo subdirectory** (`/dataplane`). Sibling dirs:
- `proto/` — Control-plane Protobuf source definitions
- `gateway/` — Control plane (Go)
- `dashboard/`, `website/` — UI

`ntgw-proto` build scripts compile local Envoy/google protos from
`crates/ntgw-proto/proto` with `protoc-bin-vendored`. Checked-in BSR-generated
control-plane Rust code is verified separately with
`scripts/verify-bsr-generated.sh`.

### Crate Dependency Map

```
ntgw-app (binary) — orchestrates everything
├── ntgw-config       — YAML config, file watching
├── ntgw-http         — HTTP/gRPC proxy (Pingora-based), filters, sessions, cache
│   ├── ntgw-ai       — AI Gateway proxy (rate limiting, multi-format)
│   ├── ntgw-wasm     — wasmtime 44 plugin engine
│   │   └── ntgw-wasm-sdk
│   ├── ntgw-ir       — Runtime IR, route matching, LB, fast-path
│   │   └── ntgw-proto — Protobuf codegen
│   └── ntgw-observability — Metrics, tracing, OTel
├── ntgw-stream       — TCP/UDP/TLS stream proxy
├── ntgw-xds          — xDS client for control plane
├── ntgw-shared-tls   — TLS config / certs
└── ntgw-allocator    — Memory allocator helpers (mimalloc/jemalloc)
```

### Key Dependencies
- **Pingora 0.8.0** — Core proxy framework (Cloudflare). Used for HTTP/stream proxy runtime.
- **tokio** (full) — Async runtime
- **tonic** — gRPC (xDS client, ext auth)
- **axum** — Admin API server
- **wasmtime 44** — Wasm plugin engine
- **OpenTelemetry** — Metrics and tracing

## Code Conventions

- **`#![forbid(unsafe_code)]`** — Present in `ntgw-app`, `ntgw-proto`, `ntgw-ir`, and others. Do not add unsafe code.
- **Workspace dependencies** — All shared deps declared in root `Cargo.toml` under `[workspace.dependencies]`. Use `{workspace = true}` in crate Cargo.tomls.
- **Edition 2024**, **Apache-2.0** license.

### Test Patterns

Two test tiers with different placement rules:

**Integration tests** (`tests/` directory at crate root) — Standard Rust integration
tests compiled as separate binaries. Use when tests only need the crate's public
API and benefit from real-world end-to-end scenarios.

Crates using this pattern: `ntgw-ai` (22 test files), `ntgw-ir` (15 test files),
`ntgw-wasm` (2 test files).

**Unit tests** (`src/tests/` or `src/<module>/tests/`) — `#[cfg(test)]` modules
inside the source tree. Use when tests need access to private internals. Two
composition styles:

- Standard `mod` declarations: `ntgw-config/src/tests/mod.rs` declares `mod
  basics; mod config_load; …` — each sub-file is a standard Rust module.
- `include!()` composition: `ntgw-xds/src/tests/runtime_apply.rs` pulls in test
  files via `include!("runtime_apply/apply_result.rs");` — keeps all test code
  within a single `#[cfg(test)]` module, useful when tests share many helpers.

Deep sub-module tests in `src/<module>/tests/` follow the same conventions and
are co-located with the code they test (e.g. `ntgw-http/src/session/tests/`,
`ntgw-stream/src/tcp/tests/`).

**Additional patterns:**
- `proptest` for property-based testing in `ntgw-ir`, `ntgw-http`, `ntgw-stream`
- `h2` crate used for HTTP/2 test fixtures in `ntgw-http`

## CI (GitHub Actions)

5 jobs run on `ubuntu-latest`:
1. `cargo check --workspace`
2. `cargo test --workspace`
3. `cargo clippy --workspace -- -D warnings`
4. `cargo fmt --all -- --check`
5. `scripts/verify-bsr-generated.sh`

The Rust jobs do not require system `protoc`; `ntgw-proto` uses
`protoc-bin-vendored` for local Envoy/google protos. The `proto-check` job uses
Buf to verify checked-in BSR-generated control-plane Rust code.

## Docker

- Build context for normal local builds is the workspace root (`/root/nantian-gw`), not `dataplane/`.
- Local build command: `docker build -f dataplane/Dockerfile -t ntgw-app .`
- `scripts/verify-docker-build.sh` creates the same synthetic context shape used by GitHub Actions: `<context>/dataplane`.
- The Dockerfile uses `cargo-chef` stages:
  1. `chef` installs native build dependencies and `cargo-chef`
  2. `planner` creates `recipe.json`
  3. `builder` cooks dependency layers, then builds `ntgw-app`
  4. runtime copies `/usr/local/bin/ntgw-app`
- Do not add system `protobuf-compiler`; `ntgw-proto` uses `protoc-bin-vendored`.
- Required native build packages remain `cmake`, `pkg-config`, `clang`, `make`, and `g++`.
- Default build feature: `allocator-jemalloc` through `DATAPLANE_CARGO_FEATURES`.
- Binary: `ntgw-app` at `/usr/local/bin/ntgw-app`.

## Release Profile

```toml
[profile.release]
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
```

## Naming Conventions

### Config vs Options

Use two distinct suffixes for configuration structs depending on where they are used:

- **`Config`** — Deserialized from YAML or other persistent configuration sources. These types implement `serde::Deserialize` (and often `Serialize`) and live primarily in `ntgw-config`. They represent the user-facing, file-based configuration surface.

  Examples: `DataPlaneConfig`, `LogConfig`, `AccessLogConfig`, `AdminAuthConfig`, `RuntimeConfig`, `SessionPersistenceConfig`, `XdsTlsConfig`, `ExperimentalConfig`.

  Isolated `Config` types outside `ntgw-config` (e.g., `AdminRuntimeConfig` in `ntgw-app`, `RateLimitConfig` in `ntgw-ai`) follow the same rule: they represent configuration data that is either derived from the file config or consumed as structured input by a subsystem.

- **`Options`** — Runtime parameters passed to constructors at startup. These types aggregate the settings a subsystem needs to operate, are typically cloned or `Arc`-wrapped, and are NOT deserialized directly from config files. Builders consume file-based `*Config` values and produce `*Options`.

  Examples: `RuntimeOptions`, `ConnectOptions`, `TransportOptions`, `ClientTlsOptions`, `SessionPersistenceOptions`, `AccessLogOptions`, `HttpAdmissionOptions`, `HttpCircuitBreakerOptions`, `HttpRateLimitOptions`, `RetryBudgetOptions`.

**Rule of thumb**: If it comes from a file → `Config`. If it is handed to a subsystem constructor → `Options`.

## Known Issues

- **prometheus 0.13** is pinned (not workspace-managed) in `ntgw-http` and `ntgw-ai`. Upstream `pingora-core 0.8.0` pulls `prometheus 0.13.x` → `protobuf 2.x`. Tracked as `RUSTSEC-2024-0437` in `deny.toml` — the dataplane only exports Prometheus text format, no attacker-supplied protobuf parsing.
- **camelCase YAML convention gap (P1)** — Dataplane config structs use `#[serde(rename_all = "camelCase")]`, meaning YAML keys are camelCase while Rust fields are snake_case. This is non-idiomatic for YAML configs (industry standard is snake_case). See [Config Naming Convention Gap](#config-naming-convention-gap) below for full audit and migration plan.
- **Session persistence secret (P2)** — The default config (`SessionPersistenceConfig`) has empty `secret_key` and `secret_key_file` fields. Without configuring a shared secret, the dataplane auto-generates a key at startup and logs: `"session persistence using auto-generated key; configure sharedSecret or sharedSecretFile for multi-replica deployments"`. Impact: (1) single-instance restarts lose all active sessions, and (2) replicas cannot share session state. For multi-replica deployments, this is a **required** configuration item. The Helm chart provides `sessionPersistence.existingSecret` and `sessionPersistence.sharedSecret` in `values.yaml` to wire a pre-shared key.
- **Large files (P2)** — 250 LOC soft cap is aspirational. After progressive splitting (2026-07-27), the following remain above 500 LOC as "split to the point of diminishing returns":
  - `ntgw-ir/src/lib.rs` (984 LOC) — public type definitions; splitting would break workspace-wide API
  - `ntgw-ir/src/snapshot/backend_resolution/mod.rs` (854 LOC) — already modularized; mod.rs re-exports
  - `ntgw-http/src/proxy/filters/mod.rs` (838 LOC) — `do_request_filter` pipeline; tightly cohesive
  - `ntgw-http/src/proxy/mod.rs` (991 LOC) — `ProxyHttp` trait impl; lifecycle hooks form single unit
  - `ntgw-http/src/proxy/logging/mod.rs` (664 LOC) — bulk is `#[cfg(test)]` module
  - `ntgw-http/src/proxy/context.rs` (636 LOC) — request context lifecycle
  - `ntgw-ai/src/observability/langfuse.rs` (836 LOC) — single-concern Langfuse integration
  - `ntgw-stream/src/tcp.rs` (664 LOC) — TCP proxy handling
  Previously split (2026-07-27): `proxy/request.rs` (859→9 files), `proxy/logging.rs` (884→5 files).

## Config Naming Convention Gap

**Status**: Steps 1-5 completed (2026-07-18). Step 6 (remove camelCase support) deferred to future release.

- Step 1 ✅: Added `#[serde(alias)]` to 21 structs (132 aliases)
- Step 2 ✅: Rewrote `configs/dataplane/*.yaml` with snake_case keys
- Step 3 ✅: Updated inline YAML in test fixtures
- Step 4 ✅: Added regex-based camelCase detection + warning in `DataPlaneConfig::parse_yaml()`
- Step 5 ✅: Updated Helm chart `values.yaml` + `_helpers.tpl` to emit snake_case
- Step 6: Remove `#[serde(rename_all = "camelCase")]` from all 21 structs (deferred — 1-2 releases)

### Scope

The camelCase convention for YAML/JSON serialization spans four categories with different migration feasibility:

#### Category 1: YAML config structs (`ntgw-config`) — MIGRATION TARGET

These 21 structs in `crates/ntgw-config/src/lib.rs` all use `#[serde(rename_all = "camelCase")]`:

| Struct | Fields | YAML Path |
|--------|--------|-----------|
| `DataPlaneConfig` | 6 (identity fields) | root |
| `LogConfig` | 10 | `log.*` |
| `OpenTelemetryConfig` | 7 | `log.openTelemetry.*` |
| `SentryConfig` | 7 | `log.sentry.*` |
| `AccessLogConfig` | 8 | `accessLog.*` |
| `AdminAuthConfig` | 2 | `adminAuth.*` |
| `RuntimeConfig` | 5 | `runtime.*` |
| `SessionPersistenceConfig` | 3 | `sessionPersistence.*` |
| `XdsTlsConfig` | 5 | `xdsTls.*` |
| `XdsTransportConfig` | 9 | `xdsTransport.*` |
| `RuntimeProtectionConfig` | 16 | `runtimeProtection.*` |
| `HttpCapacityConfig` | 4 | `runtimeTuning.httpCapacity.*` |
| `RuntimeTuningConfig` | 29 | `runtimeTuning.*` |
| `HttpCacheConfig` | 4 | `runtimeTuning.httpCache.*` |
| `ExperimentalConfig` | 3 | `experimental.*` |
| `RoutePolicyConfig` | 4 | xDS-sourced route policies |
| `RoutePolicyTimeoutConfig` | 4 | xDS-sourced |
| `RoutePolicyBodyLimitConfig` | 3 | xDS-sourced |
| `RoutePolicyProxyConfig` | 4 | xDS-sourced |
| `RoutePolicyConnectionConfig` | 5 | xDS-sourced |
| `TcpKeepaliveConfig` | 5 | `*TcpKeepalive.*` |

**Total**: ~130 camelCase YAML keys across 21 structs.

#### Category 2: Observability snapshots (`ntgw-observability`) — SERIALIZE-ONLY, JSON API

These 11+ structs are `Serialize`-only (no `Deserialize`), outputting JSON via admin API:

- `HttpCircuitBreakerSnapshot` (4 fields)
- `RetryBudgetSnapshot` (7 fields)
- `UdpSessionSnapshot` (8 fields)
- `RateLimitScopeSnapshot` (4 fields)
- `NamedRateLimitScopeSnapshot` (5 fields)
- `HttpRateLimitSnapshot` (10 fields)
- `AdminRequestStatsSnapshot` (1 field)
- `AdminRequestMetricSeries` (4 fields)
- `AdminRequestDurationBucket` (2 fields)
- `OverloadSnapshot` (20+ fields)
- `AccessLogMode` enum (2 variants, serialize + deserialize)

**Verdict**: These are admin API responses. Changing them is a **breaking API change** for consumers of the admin JSON API. Lower priority than Category 1. Could migrate with API versioning.

#### Category 3: External API compatibility — DO NOT CHANGE

These serialize to camelCase for compatibility with external API specifications:

- `Filter` / `CorsFilter` in `ntgw-ir` — Gateway API spec uses camelCase JSON
- `ListenerRuntimeStatus`, `ListenerListQuery`, `RouteListQuery`, `BackendListQuery` in `ntgw-app/src/admin/types.rs` — admin API query params use camelCase
- `LangfuseTracePayload` in `ntgw-ai` — Langfuse API requires camelCase

**Verdict**: These are dictated by external API contracts. Do not change.

#### Category 4: Individual field renaming — MIXED

- `WasmHook` enum variants (`#[serde(rename = "on_request")]`) — fixed spec names, do not change
- `Filter.filter_type` (`#[serde(rename = "type")]`) — JSON field name conflict, do not change
- `AccessLogRecord` (serialize + deserialize, ~18 fields) — also a JSON API concern

### Files Using camelCase YAML

| File | Lines | Status |
|------|-------|--------|
| `configs/dataplane/config.yaml` | 135 | Bundled default |
| `configs/dataplane/config.production.yaml` | 105 | Bundled production |
| `helm-charts/charts/nantian-gw/values.yaml` (L398-462) | ~65 | Helm values, generates ConfigMap YAML |
| Test fixtures in `crates/ntgw-config/src/tests/*.rs` | ~20 inline YAML blocks | Inline test YAML |

### Migration Plan

1. **Add snake_case aliases**: Add `#[serde(alias = "snake_case_name")]` to each field OR use `#[serde(rename_all = "camelCase")]` + `#[serde(alias)]` at the struct level. Both camelCase and snake_case YAML keys accepted during transition.

2. **Update bundled configs**: Rewrite `configs/dataplane/*.yaml` with snake_case keys.

3. **Update test fixtures**: Rewrite all inline YAML in test files.

4. **Log deprecation warnings**: Log a warning (rate-limited) when camelCase keys are used, pointing to the new snake_case equivalent.

5. **Update Helm chart**: Rewrite `values.yaml` `dataplane.config` section to emit snake_case YAML.

6. **Remove camelCase support**: After a deprecation period (1-2 releases), remove `#[serde(rename_all = "camelCase")]` and rely on Rust's default snake_case serialization.

### Estimated Effort

- Category 1 migration (YAML configs): ~2 days including test updates
- Category 2 migration (admin API): ~1 day + API version coordination
- Docs and Helm chart: ~0.5 day

## Error Type Audit

**Status**: Audited 2026-07-14. Only JwtError fix applied; full typed-error migration pending.

### Findings Summary

| Crate | Rating | anyhow uses (prod) | thiserror? | Custom error types |
|-------|--------|---------------------|------------|--------------------|
| `ntgw-ir` | **GOOD** | 0 | — | `BackendSelectionError` (data enum) |
| `ntgw-ai` | MIXED | 33 (8 files) | `AIError` (16 variants) | wraps `anyhow::Error` as `Internal` variant |
| `ntgw-wasm` | MIXED | 3 (2 files) | `WasmError` (11 variants) | clean, anyhow only in `mem.rs` |
| `ntgw-app` | MIXED | 6 (4 files) | — | `ApiError` (no `Error` trait) |
| `ntgw-config` | MIXED | 2 (2 files) | — | — |
| `ntgw-observability` | MIXED | 5 (5 files) | — | — |
| `ntgw-bench` | MIXED | 17 (10 files) | — | — (bench binary, acceptable) |
| `ntgw-http` | MIXED | 13 `anyhow!()` + 29 `.context()` (6 files) | — | `JwtError` (5 variants, ✅ `Error` impl added) |
| `ntgw-stream` | **POOR** | 5 `anyhow!()` + 18 func sigs (5 files) | — | — (no typed errors) |
| `ntgw-xds` | **POOR** | 5 (4 files) | — | — (public API returns `anyhow::Result`) |
| `ntgw-shared-tls` | **POOR** | 5 (5 files) | — | — (no typed errors) |
| `ntgw-proto` | NONE | 0 | — | — |
| `ntgw-wasm-sdk` | NONE | 0 | — | — |
| `ntgw-allocator` | NONE | 0 | — | — |

### Applied Fixes

- **`JwtError` in `ntgw-http/src/filters/jwt.rs`**: Added `impl std::error::Error for JwtError {}` (had `Debug` + `Display` but was missing `Error` trait). Now compatible with `Box<dyn Error>` and `pingora::Error::because()`.

### Migration Priorities

1. **ntgw-stream** (5 `anyhow!()` + 18 function signatures): Create `StreamError` enum. Replace route-mismatch, preface-timeout, preface-closed, dispatcher-stopped errors.
2. **ntgw-xds** (5 production uses, public API): Create `XdsError` enum for `ConnectionFailed`, `StreamError`, `PreflightFailed`, `TlsConfig`. Fixes public API return type.
3. **ntgw-shared-tls** (5 production uses): Create `SharedTlsError` for `CertificateLoad`, `HandshakeFailure`, `BindFailure`, `IdentityConfig`.
4. **ntgw-http** (13 `anyhow!()` in session.rs + TLS handling): Create `SessionError` and `TlsIdentityError` enums.
5. **ntgw-ai** (19 langfuse.rs uses): Convert langfuse anyhow uses to `AIError` variants or dedicated `LangfuseError`.

## Tracing Coverage

**Status**: Audited 2026-07-14. TCP span added; no other instrumentation added.

### Macro Usage by Crate

| Crate | info_span! | info! | debug! | warn! | error! | trace! | #[instrument] | Span API |
|-------|-----------|-------|--------|-------|--------|--------|---------------|----------|
| `ntgw-http` | **1** | 7 | 2 | 36 | 6 | 0 | 0 | `.record()` ×3, `field::Empty` ×15 |
| `ntgw-ai` | 3 | 3 | 2 | 10 | 0 | 0 | 0 | — |
| `ntgw-stream` | **1** (new) | 5 | 14 | 13 | 3 | 0 | 0 | — |
| `ntgw-app` | 0 | 3 | 0 | 7 | 15 | 0 | 0 | — |
| `ntgw-wasm` | 0 | 4 | 5 | 9 | 0 | 0 | 0 | — |
| `ntgw-xds` | 0 | 1 | 4 | 4 | 0 | 0 | 0 | — |
| `ntgw-shared-tls` | 0 | 2 | 0 | 6 | 1 | 0 | 0 | — |
| `ntgw-observability` | 0 | 0 | 0 | 4 | 1 | 0 | 0 | — |
| `ntgw-ir` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | — |
| `ntgw-config` | 0 | 0 | 0 | 0 | 0 | 0 | 0 | — |
| **Total** | **5** | **25** | **27** | **89** | **26** | **0** | **0** | **3 record + 15 field** |

### Key Gaps

- **No `#[tracing::instrument]` anywhere** — zero use of the procedural macro for automatic span creation on async functions.
- **No `debug_span!`, `error_span!`, `warn_span!`, `trace_span!`** — only `info_span!` is used.
- **No `trace!` level logging** — completely unused, even in high-cardinality paths.
- **Only 1 span in HTTP proxy** — `ntgw-http/src/proxy/request.rs` creates a per-request span with 15 fields. Filter execution, retry, and downstream/upstream I/O run outside any span.
- **ntgw-stream had zero spans** — TCP and UDP proxy hot paths had no span instrumentation. Added 1 span in `tcp::handle_connection`.

### Applied Fixes

- **TCP connection span in `ntgw-stream/src/tcp.rs`**: Added `info_span!("tcp_connection", %listener_name)` wrapping `handle_connection()`. Every TCP proxy connection now has a named span. This adds observability to the busiest hot path with minimal overhead (~no-op until subscriber samples).

## Config Hot-Reload Audit (P1)

**Status**: Audited 2026-07-14. No changes made; documentation only.

### Current Architecture

There are **three distinct config reload mechanisms** at different layers:

#### Layer 1: YAML File Config (`ntgw-config/src/reload.rs`)

`ReloadingDataPlaneConfig` wraps `Arc<RwLock<CachedConfig>>`. On each `load()` or `load_if_changed()`:

1. Fast path: acquire read lock, check if cached value is fresh enough (`refresh_interval` elapsed?). If not, return cached.
2. Slow path: acquire write lock, re-check freshness (double-checked locking), compare file stamp (mtime + size).
3. If file changed: re-parse YAML, update cache in-place, return new value.

**Key detail**: Uses `std::sync::RwLock` (from std, not `parking_lot`). Readers block during writes. This is NOT lock-free. On config reload, any concurrent `load()` caller waits for the write lock to be released.

#### Layer 2: Runtime Config Propagation (`ntgw-app/src/config_reload.rs`)

The `spawn_config_reload_loop` task watches the config file (inotify + 30s watchdog polling fallback), calls `ReloadingDataPlaneConfig::load_if_changed()` on each change, builds a `ConfigSnapshot`, and pushes it to subsystems via:

- `tokio::sync::watch::Sender<Arc<Config>>` — for HTTP, stream, shared-TLS, xDS, active-health configs (lock-free for readers via `watch::Receiver::borrow()`)
- `Arc<RwLock<Config>>` (parking_lot) — for admin, circuit-breaker, rate-limit, retry-budget (requires write lock to update)

The `watch` channels are the modern approach — readers get a shared `Arc` clone without any locking.

#### Layer 3: Snapshot (IR) Switching (`ntgw-ir/src/lib.rs`)

`SharedSnapshot = Arc<ArcSwap<Snapshot>>`. This is the runtime IR used for per-request route matching. **This already uses `arc-swap` for truly lock-free readers.** The `ArcSwap::store(Arc::new(snapshot))` atomically swaps the pointer — readers see either the old or new snapshot, never a partial state.

### Gap Analysis

| Area | Mechanism | Lock-Free? | Assessment |
|------|-----------|------------|------------|
| YAML config reload | `std::sync::RwLock` | No | Readers block on write. Low impact — config reloads are rare (human-scale). |
| Subsystem config broadcast | `tokio::sync::watch` | Yes (readers) | ✅ Good. Uses Arc clone, no locking on read path. |
| Subsystem mutable state | `parking_lot::RwLock` | No | Admin/circuit_breaker/rate_limit/retry_budget use write locks on reload. Low contention. |
| Snapshot (IR) switching | `arc_swap::ArcSwap` | **Yes** | ✅ Already optimal. The hottest path (per-request route matching) reads snapshots without any lock. |

### Recommendation

The `RwLock` in `ReloadingDataPlaneConfig` is **not a performance concern** because:
- Config reload is a rare operation (seconds to minutes between reloads)
- The `refresh_interval` guard ensures readers don't hammer the lock
- The critical hot path (snapshot lookup) already uses `ArcSwap`

**No changes recommended for the RwLock in ntgw-config.** The architecture is well-layered: slow operations use locks, hot paths are lock-free.

### Files Involved

| File | Role |
|------|------|
| `crates/ntgw-config/src/reload.rs` | YAML file config reload with RwLock |
| `crates/ntgw-config/src/lib.rs` | `DataPlaneConfig` struct (YAML deserialization) |
| `crates/ntgw-config/src/impls.rs` | Config methods (load, env overrides) |
| `crates/ntgw-app/src/config_reload.rs` | File watcher + config broadcast loop |
| `crates/ntgw-ir/src/lib.rs` L31: `use arc_swap::ArcSwap;` L62: `type SharedSnapshot = Arc<ArcSwap<Snapshot>>;` | Lock-free snapshot switching |
| `crates/ntgw-ir/src/snapshot.rs` L22-24: `Snapshot::shared()` | Creates `ArcSwap`-wrapped snapshot |

## Route Matching Performance Audit (P2)

**Status**: Audited 2026-07-14. No changes made; documentation only.

### Current Architecture

Route matching uses a **three-tier** strategy: hash-based index narrowing → fast-path linear scan → slow-path hostname-index + linear scoring.

#### Tier 1: Runtime Indexes (built in `rebuild_runtime_indexes`)

Pre-built hash maps narrow the search space:

| Index | Type | Key → Value |
|-------|------|------------|
| `listener_name_index` | `HashMap<String, usize>` | Listener name → index |
| `http_listener_port_index` | `HashMap<u32, Vec<usize>>` | Port → listener indices |
| `http_route_hostname_index` | `HostnameRouteIndex` (see below) | Hostname → route indices |
| `grpc_route_hostname_index` | `HostnameRouteIndex` | Hostname → route indices |
| `stream_listener_route_index` | `HashMap<String, Vec<usize>>` | Listener name → stream route indices |
| `route_attachment_listener_index` | `RouteAttachmentListenerIndex` (HashMap) | Namespace+name → listener indices |
| `backend_service_index` | `HashMap<String, BackendServiceBucket>` | Service name → backend entries |
| `backend_index` | `HashMap<Arc<str>, usize>` | Backend name → index |

#### Tier 2: HostnameRouteIndex

`HostnameRouteIndex` is a **custom hash-based hostname matcher** (not a Trie):

```
struct HostnameRouteIndex {
    catch_all: Vec<usize>,           // routes with no hostname filter
    exact: HashMap<String, Vec<usize>>,  // exact hostname matches
    wildcard_suffix: HashMap<String, Vec<usize>>,  // wildcard suffix ("*.example.com" → "example.com")
}
```

For a request host like `foo.bar.example.com`, it checks:
1. `exact["foo.bar.example.com"]`
2. `wildcard_suffix["bar.example.com"]` (suffix of `*.bar.example.com`)
3. `wildcard_suffix["example.com"]` (suffix of `*.example.com`)
4. `wildcard_suffix["com"]` (suffix of `*.com` — unlikely match)
5. `catch_all`

Uses **merged-cell iteration** (`next_candidate_index_after_normalized`) to return route indices in order without duplicates across multiple HashMap matches. Uses `partition_point` for O(log n) skip-ahead within each vector.

#### Tier 3: Linear Scan with Scoring

After narrowing by hostname index, the system does a **linear scan** over candidate routes, computing a score for each match and keeping the best one:

```
HTTP scoring (HttpCandidateScore):
  listener_host_score + route_host_score + rule_score
    where rule_score = path_rank (Exact=3 > Regex=2 > Prefix=1) 
                      + path_length 
                      + method_specified

Stream scoring (StreamMatchScore):
  hostname_rank (exact=2 > wildcard=1) + hostname_length

gRPC scoring (GrpcCandidateScore):
  listener_host_score + route_host_score + service_score + method_score
```

#### Fast Path (`http_fast_path.rs`, `stream_fast_path.rs`)

For **simple routes** (no filters, no retries, no timeouts, no session persistence, no header/query matching), the system pre-compiles a `HttpFastPathPlan` / `StreamFastPathPlan` during index building. This pre-compiles:
- All pre-resolved backend references (endpoint + runtime IDs)
- Flat vectors of compiled rules per route
- Listener→route attachment mappings

The fast path selection does a **pure linear scan** over these pre-compiled structures — no hash map lookups in the hot path. This is optimal for the common case where most routes are simple.

### Design Analysis

| Aspect | Approach | Notable |
|--------|----------|---------|
| Hostname matching | Hash-based exact + wildcard suffix map | O(1) average for exact; O(k) for wildcard suffixes (k = number of `.` in hostname) |
| Path matching | Linear scan with scoring | O(n) where n = candidate routes after hostname narrowing |
| Regex compilation | Pre-compiled at index-build time (`compile_runtime_state`) | Regexes stored as `Option<Arc<Regex>>`, only compiled once |
| Fast path | Flat vectors, no HashMap lookups | Bypasses ALL index lookups for simple routes; pure sequential scan |
| Merge iteration | Deduplication via `partition_point` | Prevents visiting the same route index twice across different hostname matches |
| Header matching | Loop over expected headers, HashMap lookup per header | O(h × log v) where h = expected headers, v = values per header |

### Trie vs Hash-Based

**No character-based Trie is used.** The hostname index uses HashMaps, not a prefix tree. This is actually **correct for this domain** because:

1. Hostname matching in HTTP routing is NOT longest-prefix — it's **exact match or wildcard suffix** (`*.example.com`). A Trie would not help here.
2. Path matching uses linear scan with scoring, not Trie lookup. The scoring is needed because Gateway API defines priority ordering (Exact > Prefix > Regex), which doesn't fit a pure Trie.
3. The fast path avoids even HashMap lookups.

A Trie *could* benefit path prefix matching if there are hundreds of routes with the same hostname but different path prefixes, but:
- The `PathPrefix` match has a simple `starts_with` check — already O(min(len(prefix), len(path)))
- With hostname narrowing, the number of candidate routes per request is typically small (single digits)
- The scoring system already provides correct priority ordering

### Recommendation

**No changes recommended.** The architecture is well-tuned for the problem domain:
- Hash-based hostname narrowing reduces the candidate set to single digits in most configurations
- Fast path compilation eliminates all overhead for simple routes
- The linear scan over a small candidate set is negligible
- Regex pre-compilation eliminates repeated regex parsing

A Trie would be a premature optimization that would add complexity without measurable benefit for realistic route table sizes (tens to low hundreds of routes).

### Files Involved

| File | Role |
|------|------|
| `crates/ntgw-ir/src/matching.rs` | Low-level match functions (HTTP path, headers, query, gRPC, stream, hostname) |
| `crates/ntgw-ir/src/http_fast_path.rs` | Fast path: pre-compiled HTTP route selection (no index lookups) |
| `crates/ntgw-ir/src/stream_fast_path.rs` | Fast path: pre-compiled stream route selection |
| `crates/ntgw-ir/src/http_selection/candidates.rs` | Listener/route candidate generation using hostname indexes |
| `crates/ntgw-ir/src/http_selection/scoring.rs` | Scoring logic for HTTP/gRPC route matching |
| `crates/ntgw-ir/src/http_selection/selection.rs` | Full HTTP/gRPC route selection loop |
| `crates/ntgw-ir/src/snapshot.rs` | `rebuild_runtime_indexes()` — builds all hash indexes and fast path plans |
| `crates/ntgw-ir/src/lib.rs` L391-453 | `HostnameRouteIndex` definition and merged-cell iteration |
