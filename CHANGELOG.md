# Changelog — Aether Gateway Data Plane

## Unreleased

### Core Proxy
- Rust proxy runtime with HTTP/1.1 and HTTP/2 support.
- xDS client with full snapshot + partial rebuild optimization.
- Admin API at `127.0.0.1:19080` with health, metrics, and config endpoints.
- Prometheus metrics integration with Grafana dashboard.

### Wasm Plugin System (`aeg-wasm`)
- wasmtime-based plugin runtime for custom request/response hooks.
- `aeg-wasm-sdk`: Rust SDK for writing plugins with host function bindings.
- `PluginManager`: lifecycle management (load, invoke, unload).
- AI inference sandbox for tokenizer/embedder execution in Wasm.
- Prebuilt wasm modules with CI build integration.

### Performance & Stability
- Connection pool management and backend health checking.
- Request buffering and streaming support.
- Backend TLS with certificate validation.

No formal releases yet. See the main [Aether Gateway
CHANGELOG.md](../aether-gateway/CHANGELOG.md) for project-wide release
history.