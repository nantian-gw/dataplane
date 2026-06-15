# ntgw-stream Runtime Unwrap Governance

Date: 2026-06-15

This note records the multi-target guardrail expansion that adds
`crates/ntgw-stream/src/` production code to dataplane runtime unwrap
governance.

Governed scope:

- `crates/ntgw-stream/src/` production Rust code

Current audit conclusion:

- production sources under `crates/ntgw-stream/src/` are clean under the
  runtime unwrap scanner
- broad grep hits currently remain in test-only code such as:
  - `crates/ntgw-stream/src/access_log.rs`
  - `crates/ntgw-stream/src/pool.rs`
  - `crates/ntgw-stream/src/sni/tests.rs`
  - `crates/ntgw-stream/src/udp/tests.rs`

Out of scope for this guardrail:

- inline `#[cfg(test)]` items and modules
- files reached only through `#[cfg(test)] mod ...;`
- standalone test-only paths outside governed production code
