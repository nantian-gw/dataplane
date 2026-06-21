# Cache Hit Access Log Wait Fix Spec

## Problem

GitHub Actions CI run `27902194981` failed in the `Test` job while running
`cargo test --workspace`. The failing test was
`runtime::tests::cache_hit_preserves_sent_response_access_log_headers`, with:

```text
assertion failed: log_contents.contains("text/plain -")
```

The test performs two requests against a cacheable route. The first request
hits upstream and should emit `text/plain 200`; the second request should be
served from cache and should emit `text/plain -`. The final assertion currently
uses `wait_for_log_contents`, which returns as soon as the log file is non-empty.
On CI, that can return after the first access log line is flushed but before
the second cache-hit line is flushed.

## Goals

- Make the cache-hit access log test wait until both expected log lines are
  present before asserting.
- Preserve the existing `wait_for_log_contents` behavior for tests that only
  need any non-empty log output.
- Add a deterministic helper-level regression test proving predicate-based log
  waiting does not return the first non-empty contents too early.
- Keep the fix test-only; do not change dataplane runtime, proxy, cache, access
  log writer, protobuf, Helm, dashboard, website, or gateway behavior.

## Non-Goals

- Do not increase global sleeps, fixed timeouts, or CI retries.
- Do not weaken the assertions in
  `cache_hit_preserves_sent_response_access_log_headers`.
- Do not change access log formatting or `$upstream_status` rendering.

## Design

Add `wait_for_log_contents_matching(path, predicate)` in
`crates/ntgw-http/src/runtime/tests_support_helpers.rs`. The helper will poll
the log file until the caller-provided predicate returns true. It will keep the
last observed contents so timeout failures preserve useful assertion context.

Refactor `wait_for_log_contents(path)` to delegate to the new helper with the
existing non-empty predicate. Update only the cache-hit access log test's final
read to wait for both `text/plain 200` and `text/plain -`.

Add a focused async test in `tests_support_helpers.rs` that writes an initial
non-matching log line, appends the matching log line after a short delay, and
verifies the predicate helper returns the combined content rather than the
initial non-empty content.

## Acceptance Criteria

- In the dataplane worktree,
  `cargo test -p ntgw-http wait_for_log_contents_matching_waits_for_predicate -- --nocapture`
  passes.
- In the dataplane worktree,
  `cargo test -p ntgw-http runtime::tests::cache_hit_preserves_sent_response_access_log_headers -- --nocapture`
  passes.
- In the dataplane worktree, `cargo test -p ntgw-http --lib` passes.
- In the dataplane worktree, `cargo test --workspace` passes.
- In the dataplane worktree, `cargo clippy --workspace -- -D warnings` passes.
- In the dataplane worktree, `cargo fmt --all -- --check` passes.
- In the dataplane worktree, `git diff --check` passes.
- The main checkout's pre-existing untracked files remain untouched.
- Sibling repositories `gateway`, `dashboard`, `website`, `proto`, and
  `helm-charts` are not modified by this task.
