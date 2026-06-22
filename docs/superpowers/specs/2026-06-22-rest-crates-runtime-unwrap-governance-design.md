# Rest Crates Runtime Unwrap Governance Design

Date: 2026-06-22

## Goal

Extend dataplane runtime unwrap governance beyond the already-governed
`ntgw-ai` and `ntgw-stream` production sources so every remaining crate is
covered by the same `scripts/audit-runtime-unwraps.sh --enforce` guardrail.

## Current State

The existing guardrail scans production Rust sources for `.unwrap()` and
`.expect()` calls while excluding test-only code. It currently governs:

- `crates/ntgw-ai/src`
- `crates/ntgw-stream/src`

The scanner already masks comments and string literals and excludes simple
`#[cfg(test)]` items/modules and files reached through `#[cfg(test)] mod ...;`.
Two gaps block a clean expansion to the remaining crates:

- test files pulled in through `include!("...")` from an excluded test module
  are still scanned as production
- cfg expressions such as `#[cfg(all(test, target_os = "linux"))]` are not
  treated as test-only

After accounting for those test-only patterns, the remaining true production
hits are the global Wasm engine/plugin-manager initialization paths in
`ntgw-wasm`.

## Scope

Govern these additional production roots:

- `crates/ntgw-allocator/src`
- `crates/ntgw-app/src`
- `crates/ntgw-bench/src`
- `crates/ntgw-config/src`
- `crates/ntgw-http/src`
- `crates/ntgw-ir/src`
- `crates/ntgw-observability/src`
- `crates/ntgw-proto/src`
- `crates/ntgw-shared-tls/src`
- `crates/ntgw-wasm/src`
- `crates/ntgw-wasm-sdk/src`
- `crates/ntgw-xds/src`

The enforcement scope remains production Rust source under each governed
`src/` tree. Test-only code remains out of scope, including standalone
`crates/*/tests/**`, inline test modules/items, files reached only through
`#[cfg(test)] mod ...;`, and files recursively included from those test-only
modules with `include!("...")`.

## Non-Goals

- Do not remove every unwrap/expect from test code in this change.
- Do not introduce `clippy::unwrap_used` or `clippy::expect_used` workspace
  denies; the existing scanner remains the canonical guardrail for this batch.
- Do not edit generated protobuf output.
- Do not touch sibling component repositories.

## Design

### Guardrail Expansion

Add an aggregate governance note,
`docs/runtime-unwrap-rest-crates-zero-tolerance.md`, with the 2026-06-22 batch
marker. Extend the target list in `scripts/audit-runtime-unwraps.sh` to cover
the remaining crate `src/` roots and validate that aggregate note.

### Scanner Test-Only Detection

Keep the current scanner structure but extend test-only exclusion in two ways:

- Treat cfg expressions that are definitely test-only as test-only attributes.
  Supported forms are `test` and `all(...)` expressions containing `test`.
  Expressions such as `any(test, feature = "...")` and `not(test)` are not
  considered definitely test-only.
- Recursively follow `include!("relative/path.rs")` from already-excluded
  test-only files and exclude those included files from production scanning.

This keeps the scanner conservative: production files are still scanned unless
the path is proven reachable only from test configuration.

### Wasm Initialization Errors

Replace the two production `expect` calls in `ntgw-wasm` global initialization
with result-returning APIs:

- `engine::global_engine() -> anyhow::Result<Arc<Engine>>`
- `WasmEngine::global() -> anyhow::Result<WasmEngine>`
- `plugin::global_plugin_manager() -> Result<Arc<PluginManager>, WasmError>`

The `OnceLock` storage should cache either the initialized singleton or the
initialization error string. Callers clone the `Arc` on success and receive a
structured error on failure. `ntgw-http` should log the error and disable the
Wasm filter for that snapshot instead of panicking.

## Acceptance Criteria

- `python3 scripts/test-audit-runtime-unwraps.py` passes and includes fixture
  coverage for recursive test `include!` exclusions, `cfg(all(test, ...))`
  exclusions, and the expanded target list.
- `scripts/audit-runtime-unwraps.sh --enforce` passes and reports clean output
  for `ntgw-ai`, `ntgw-stream`, and every newly governed crate root listed in
  this spec.
- `rg -n --glob '!crates/ntgw-ai/**' --glob '!crates/*/tests/**' '\.(unwrap|expect)\s*\(' crates`
  may still show test-only hits, but no hit may appear in the guardrail output.
- `cargo test -p ntgw-wasm` passes after changing global Wasm initialization
  APIs.
- `cargo test --workspace` passes.
- `cargo clippy --workspace -- -D warnings` passes.
- `cargo fmt --all -- --check` passes.
- `git -C /root/nantian-gw/gateway status --short`,
  `git -C /root/nantian-gw/proto status --short`,
  `git -C /root/nantian-gw/dashboard status --short`,
  `git -C /root/nantian-gw/website status --short`, and
  `git -C /root/nantian-gw/helm-charts status --short` show no changes caused
  by this task.
