# Data Plane AI Cache and Rate Limit Hot Path Optimization

## Scope

This change is limited to the data plane repository and the `ntgw-ai` crate.

## Problem

Recent AI Gateway features added correct behavior for semantic cache keys and
sliding-window rate limits, but the hot path still does avoidable work:

- expired semantic cache entries are returned as misses but remain resident until
  a future capacity-triggered prune;
- token window usage is recalculated by scanning the full deque on every
  rate-limit check and token recording operation;
- prompt-guard and content-safety keyword checks lowercase configured keywords on
  every request instead of normalizing them once at construction.

## Implementation Plan

1. Update `MemoryCacheBackend::lookup` to remove expired entries immediately
   after detecting them.
2. Add running token totals to the AI rate limiter's sliding window and maintain
   those totals while pruning/recording token events.
3. Normalize prompt-guard and content-safety keywords once at construction while
   keeping public verdict text unchanged.
4. Add regression tests for expired cache cleanup, token total accounting, and
   case-insensitive keyword behavior.

## Acceptance Criteria

- `cargo test -p ntgw-ai semantic_cache`
- `cargo test -p ntgw-ai ratelimit`
- `cargo test -p ntgw-ai prompt_guard`
- `cargo test -p ntgw-ai content_safety`
- `cargo fmt --all -- --check`
- `cargo clippy -p ntgw-ai --all-targets -- -D warnings`

Unrelated component repositories must remain unchanged.

## Validation Results

- `cargo test -p ntgw-ai semantic_cache` — passed.
- `cargo test -p ntgw-ai ratelimit` — passed.
- `cargo test -p ntgw-ai prompt_guard` — passed.
- `cargo test -p ntgw-ai content_safety` — passed.
- `cargo test -p ntgw-ai` — passed: 83 unit tests, all integration tests, and
  doc tests passed.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p ntgw-ai --all-targets -- -D warnings` — passed.
- First `cargo test --workspace` attempt failed because the local worktree
  `target/` directory exhausted the filesystem (`No space left on device`).
  Removed only this worktree's generated `target/` directory with `cargo clean`.
- `CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test --workspace`
  — passed.
- `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo clippy --workspace -- -D warnings`
  — passed.
- `scripts/verify-bsr-generated.sh` — passed.
