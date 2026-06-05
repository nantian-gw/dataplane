# Nantian Gateway Data Plane

Rust workspace providing the high-performance HTTP and stream proxy runtime for Nantian Gateway.

## Crates

| Crate | Description |
|---|---|
| `aeg-app` | Data plane binary and service composition |
| `aeg-http` | HTTP/gRPC proxy runtime and filters |
| `aeg-ai` | AI Gateway proxy with multi-format support and rate limiting |
| `aeg-wasm` | wasmtime plugin engine and host functions |
| `aeg-stream` | TCP/UDP/TLS passthrough stream proxy |
| `aeg-ir` | Runtime IR, snapshot indexing, and proto ingestion |
| `aeg-xds` | xDS client for control plane config streaming |
| `aeg-config` | Data plane configuration management |
| `aeg-observability` | Metrics, tracing, and observability infrastructure |
| `aeg-allocator` | Custom memory allocation helpers |
| `aeg-bench` | Performance benchmarks |
| `aeg-proto` | Protobuf definitions and generated code |
| `aeg-shared-tls` | Shared TLS configuration and certificate management |
| `aeg-wasm-sdk` | SDK for building Wasm plugins |

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