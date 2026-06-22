# Rest Crates Runtime Unwrap Governance

Date: 2026-06-22

This note records the zero-tolerance guardrail expansion for dataplane
production crate roots that were not already governed by the `ntgw-ai` and
`ntgw-stream` batches.

Governed scope:

- `crates/ntgw-allocator/src/`
- `crates/ntgw-app/src/`
- `crates/ntgw-bench/src/`
- `crates/ntgw-config/src/`
- `crates/ntgw-http/src/`
- `crates/ntgw-ir/src/`
- `crates/ntgw-observability/src/`
- `crates/ntgw-proto/src/`
- `crates/ntgw-shared-tls/src/`
- `crates/ntgw-wasm/src/`
- `crates/ntgw-wasm-sdk/src/`
- `crates/ntgw-xds/src/`

Current audit conclusion:

- governed production sources are clean under the runtime unwrap scanner
- broad grep hits remain allowed only in test-only code outside the governed
  production surface or behind test-only configuration

Out of scope for this guardrail:

- standalone `crates/*/tests/**` integration tests
- inline test-only items and modules
- files reached only through `#[cfg(test)] mod ...;`
- files recursively included from those test-only files with `include!("...")`
