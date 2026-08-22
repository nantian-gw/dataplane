# Dataplane Hot Path Optimizations

Date: 2026-08-22

## Scope

This change is limited to the `dataplane` repository. No sibling repositories are in scope.

## Goals

1. Reduce unnecessary response-body buffering when AI Gateway is globally enabled but the current request did not enter the AI flow.
2. Reduce avoidable per-request allocation on Wasm request hook header marshaling where practical.
3. Keep behavior stable for cache, AI post-processing, Wasm hooks, and access logging.

## Non-Goals

- Do not redesign Pingora response cache internals in this patch.
- Do not introduce unsafe code.
- Do not change public protobuf or cross-repository contracts.

## Acceptance Criteria

- `cargo fmt --all -- --check` passes in `dataplane`.
- `cargo test -p ntgw-http` passes in `dataplane`.
- `cargo test -p ntgw-wasm` passes in `dataplane` if Wasm code changes.
- `cargo clippy --locked -p ntgw-http --all-targets -- -D warnings` passes for HTTP-only changes.
- `git status --short` shows changes only under `dataplane` worktree files.

## Validation Notes

Record exact commands and results below before handoff.

- `cargo fmt --all -- --check` — passed.
- `cargo test -p ntgw-ai` — passed: 87 unit tests plus integration/doc tests.
- `cargo test -p ntgw-http` — passed: 347 unit tests plus doc tests.
- `cargo clippy --locked -p ntgw-ai --all-targets -- -D warnings` — passed.
- `cargo clippy --locked -p ntgw-http --all-targets -- -D warnings` — passed.
