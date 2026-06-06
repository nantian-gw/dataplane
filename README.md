# Nantian Gateway Data Plane

Rust workspace providing the high-performance HTTP and stream proxy runtime for Nantian Gateway.

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

## Build

```bash
cargo build --workspace
```

## Test

```bash
cargo test --workspace
```

## Lint

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```