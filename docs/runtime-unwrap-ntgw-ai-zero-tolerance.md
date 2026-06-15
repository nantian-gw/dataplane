# ntgw-ai Runtime Unwrap Governance: Second Batch

Date: 2026-06-15

This note records the second batch of the `ntgw-ai` zero-unwrap governance work.
The governed production files for this batch are:

- `crates/ntgw-ai/src/ab_test.rs`
- `crates/ntgw-ai/src/filter.rs`
- `crates/ntgw-ai/src/content_safety.rs`
- `crates/ntgw-ai/src/prompt_guard.rs`
- `crates/ntgw-ai/src/pii.rs`

`docs/runtime-unwrap-first-batch.md` remains the historical record for the
original runtime/configuration fix set.

Enforcement for this batch is limited to production code under
`crates/ntgw-ai/src/`. Test-only code remains out of scope, including
`crates/ntgw-ai/tests/**` and inline `#[cfg(test)]` modules.
