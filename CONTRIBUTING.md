# Contributing — Aether Gateway Data Plane

The data plane is the Rust proxy runtime (`dataplane/`). It handles live
HTTP/stream traffic, xDS subscriptions, admin endpoints, metrics, and the
Wasm plugin runtime.

Before opening a PR, read the main project's
[CONTRIBUTING.md](../aether-gateway/CONTRIBUTING.md),
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), and
[GOVERNANCE.md](GOVERNANCE.md).

## Development Flow

Prefer cheapest validation first:

```
cargo check --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check --all
```

## Workspace Structure

```
dataplane/
├── crates/
│   ├── aether-proxy/    # core proxy runtime
│   ├── aether-xds/      # xDS client
│   ├── aether-admin/    # admin API server
│   └── aeg-wasm-*/      # Wasm plugin system
├── proto/               # gRPC service definitions
├── Dockerfile
└── Cargo.toml           # workspace root
```

## Pull Request Expectations

- One PR, one logical change. Don't mix perf, refactor, and feature work.
- Changes to `proto/` must be verified against the Go control plane.
- Admin API changes need a compatibility summary.
- New dependencies must pass `cargo deny check`.
- Rust toolchain changes must be in `rust-toolchain.toml`.

## Questions

Use GitHub issue forms. See [SUPPORT.md](SUPPORT.md) and the main
[Aether Gateway repo](../aether-gateway/) for broader project context.