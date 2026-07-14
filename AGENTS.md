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
lto = "thin"
codegen-units = 1
panic = "abort"
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

## Config Naming Convention Gap

**Status**: Audited 2026-07-14. No renaming has been performed yet.

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
