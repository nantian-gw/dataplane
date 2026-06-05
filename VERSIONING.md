# Versioning — Aether Gateway Data Plane

The data plane follows the main [Aether Gateway versioning
policy](../aether-gateway/VERSIONING.md). This document covers
data-plane-specific considerations.

## Version Format

Semantic versioning: `vMAJOR.MINOR.PATCH`

- **MAJOR**: Breaking changes to xDS protocol, admin API contracts, or
  Wasm plugin host-function ABI.
- **MINOR**: New proxy capabilities, protocol support, observability
  endpoints, or Wasm SDK features.
- **PATCH**: Defect fixes, stability improvements, dependency updates.

Before `v1.0.0`, compatibility adjustments may occur. Strict long-term
API stability is not promised.

## Rust Toolchain

The Rust toolchain version is pinned in `rust-toolchain.toml`. Toolchain
upgrades are treated as MINOR-level changes and require:
- Full workspace build and test.
- `cargo clippy --workspace -- -D warnings`.
- `cargo deny check` passing.
- Wasm module rebuild and validation.

## Crate Versioning

Workspace crates are versioned independently but released together:
- `aether-proxy`, `aether-xds`, `aether-admin`: follow data plane release tags.
- `aeg-wasm-sdk`: may have separate pre-release cycles for plugin authors.
- `aeg-wasm-*` internal crates: follow data plane release tags.

## Release Artifacts

Data plane releases produce:
- Binary container image (`aether-gateway-dataplane`).
- Prebuilt Wasm modules.
- Cargo workspace semver tags.

See the main [VERSIONING.md](../aether-gateway/VERSIONING.md) for the
full release workflow and support scope.