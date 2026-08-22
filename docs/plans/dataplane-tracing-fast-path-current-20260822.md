# Dataplane Tracing Fast Path Optimization

Date: 2026-08-22

## Scope

This change is limited to the `dataplane` repository.

## Goal

Keep HTTP fast-path backend selection enabled when request tracing is on, as long
as route policy materialization does not require full request headers or source
IP in the selection input.

## Acceptance Criteria

- `cargo fmt --all -- --check` passes.
- `cargo test -p ntgw-http proxy::tests::fast_path` passes.
- `cargo clippy --locked -p ntgw-http --all-targets -- -D warnings` passes.
- No sibling repository changes are present.

## Validation Notes

Record exact commands and results before merge.

- `cargo fmt --all -- --check` — passed.
- `cargo test -p ntgw-http proxy::tests::fast_path` — passed: 11 selected fast-path tests.
- `cargo clippy --locked -p ntgw-http --all-targets -- -D warnings` — passed.
