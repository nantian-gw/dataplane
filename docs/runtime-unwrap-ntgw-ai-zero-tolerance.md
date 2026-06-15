# ntgw-ai Runtime Unwrap Governance: Second Batch

Date: 2026-06-15

This note records the second batch of the `ntgw-ai` zero-unwrap governance work.
The second-batch target files are:

- `crates/ntgw-ai/src/ab_test.rs`
- `crates/ntgw-ai/src/filter.rs`
- `crates/ntgw-ai/src/content_safety.rs`
- `crates/ntgw-ai/src/prompt_guard.rs`
- `crates/ntgw-ai/src/pii.rs`

`docs/runtime-unwrap-first-batch.md` remains the historical record for the
original runtime/configuration fix set.

Guardrail enforcement for this batch scans production Rust sources under
`crates/ntgw-ai/src/`, with the target files above called out as the second-batch
focus. Test-only code remains out of scope, including `crates/ntgw-ai/tests/**`,
inline `#[cfg(test)]` items and modules, and files reached only through
`#[cfg(test)] mod ...;` declarations.
