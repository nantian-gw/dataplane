# Nantian Gateway Data Plane

[![CI](https://github.com/nantian-gw/dataplane/actions/workflows/ci.yml/badge.svg)](https://github.com/nantian-gw/dataplane/actions/workflows/ci.yml)
[![Docker](https://github.com/nantian-gw/dataplane/actions/workflows/docker.yml/badge.svg)](https://github.com/nantian-gw/dataplane/actions/workflows/docker.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88.0%2B-orange.svg)](rust-toolchain.toml)

Rust workspace providing the high-performance HTTP and stream proxy runtime for Nantian Gateway.

## Architecture

```
ntgw-app (binary) — orchestrates everything
├── ntgw-config       — YAML config, file watching
├── ntgw-http         — HTTP/gRPC proxy (Pingora-based), filters, sessions, cache
│   ├── ntgw-ai       — AI Gateway proxy (rate limiting, multi-format)
│   ├── ntgw-wasm     — wasmtime 30 plugin engine
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

- Rust 1.88.0+ (see [rust-toolchain.toml](rust-toolchain.toml))
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

### Run

```bash
# Start with a config file
cargo run --release -p ntgw-app -- --config configs/ntgw.yaml
```

## Docker

Pre-built images are available on [GitHub Container Registry](https://github.com/nantian-gw/dataplane/pkgs/container/dataplane).

```bash
docker pull ghcr.io/nantian-gw/dataplane:latest
```

Build locally:

```bash
# From the monorepo root:
docker build -f dataplane/Dockerfile -t ntgw-app .
```

## License

[Apache-2.0](LICENSE)