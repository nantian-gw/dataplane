# Nantian Gateway Data Plane

[![CI](https://github.com/nantian-gw/dataplane/actions/workflows/ci.yml/badge.svg)](https://github.com/nantian-gw/dataplane/actions/workflows/ci.yml)
[![Docker](https://github.com/nantian-gw/dataplane/actions/workflows/docker.yml/badge.svg)](https://github.com/nantian-gw/dataplane/actions/workflows/docker.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.96.0-orange.svg)](rust-toolchain.toml)

High-performance Rust proxy for Nantian Gateway — HTTP, gRPC, TCP, UDP, TLS, **and AI gateway** all in one binary.

## Architecture

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

## Crates

| Crate | Description |
|---|---|
| `ntgw-app` | Data plane binary and service composition |
| `ntgw-http` | HTTP/gRPC proxy runtime and filters |
| `ntgw-ai` | AI Gateway proxy with multi-format support and rate limiting |
| `ntgw-wasm` | wasmtime plugin engine and host functions |
| `ntgw-stream` | TCP/UDP/TLS passthrough stream proxy |
| `ntgw-ir` | Runtime IR, snapshot indexing, and proto ingestion |
| `ntgw-xds` | xDS client for control plane config streaming |
| `ntgw-config` | Data plane configuration management |
| `ntgw-observability` | Metrics, tracing, and observability infrastructure |
| `ntgw-allocator` | Custom memory allocation helpers |
| `ntgw-bench` | Performance benchmarks |
| `ntgw-proto` | Protobuf definitions and generated code |
| `ntgw-shared-tls` | Shared TLS configuration and certificate management |
| `ntgw-wasm-sdk` | SDK for building Wasm plugins |

## Quick Start

### Prerequisites

- Rust 1.96.0 (see [rust-toolchain.toml](rust-toolchain.toml))
- No system `protoc` needed — `ntgw-proto` bundles its own via `protoc-bin-vendored`

### Build

```bash
# Build everything
cargo build --workspace

# Release build with jemalloc allocator
cargo build --release -p ntgw-app --features allocator-jemalloc
```

### Test

```bash
cargo test --workspace
```

### Lint

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

### Protobuf Generation Check

Checked-in control-plane Rust types under `crates/ntgw-proto/src/bsr/` are
generated from the Buf Schema Registry module:

```bash
buf generate buf.build/nantian/proto
scripts/verify-bsr-generated.sh
```

### Run

```bash
# Start with a config file
cargo run --release -p ntgw-app -- --config configs/ntgw.yaml
```

## Access Logging

Text-mode access logs support both the existing `%TOKEN%` placeholders and
Nginx-style variables in the same `accessLog.format` string.

```yaml
accessLog:
  enabled: true
  path: "stdout"
  mode: "text"
  format: '$remote_addr $request_id "$request" $status $request_time "$http_user_agent" $ntgw_backend'
```

Supported text-template forms:

- Legacy placeholders such as `%STATUS%`, `%LATENCY_MS%`, and `%UPSTREAM_CONNECT_TIME_MS%`
- Nginx-style variables such as `$remote_addr`, `$request`, `$status`, `$request_time`, `$upstream_addr`, and `$time_iso8601`
- Request-header variables using `$http_<header_name>`, for example `$http_user_agent`, `$http_referer`, and `$http_x_forwarded_for`

Notes:

- `accessLog.mode: json` keeps the stable structured JSON output and does not use `accessLog.format`
- `$http_*` captures only the named request headers needed by the effective text template
- TCP, UDP, and TLS passthrough access logs share the same template engine; unsupported HTTP-only variables render as `-`
- Route annotations can still override `enabled`, `format`, `mode`, and `sample-rate`, but not the local output path

## Docker

Pre-built images are available on [GitHub Container Registry](https://github.com/nantian-gw/dataplane/pkgs/container/dataplane).

```bash
docker pull ghcr.io/nantian-gw/dataplane:latest
```

Build locally from a Nantian workspace root where this repository is available
as `dataplane/`:

```bash
docker build -f dataplane/Dockerfile -t ntgw-app .
```

The Dockerfile uses `cargo-chef` to cache Rust dependency layers. To run the
same synthetic-context verification used by maintainers, run from this
repository:

```bash
scripts/verify-docker-build.sh
```

To run only static Dockerfile checks without a Docker daemon:

```bash
scripts/verify-docker-build.sh --static-only
```

## License

[Apache-2.0](LICENSE)
