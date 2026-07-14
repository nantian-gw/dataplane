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
