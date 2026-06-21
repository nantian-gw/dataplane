# Cache Hit Access Log Wait Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove CI flakiness from the cache-hit access log test by waiting for the exact expected log content instead of any non-empty log output.

**Architecture:** Add a predicate-based test helper for access log polling, keep the existing non-empty helper as a compatibility wrapper, and update the cache-hit test's final read to wait for both expected lines. The change is limited to `ntgw-http` test code and documentation.

**Tech Stack:** Rust 2024, Tokio async tests, `std::fs`, existing `ntgw-http` runtime test helpers.

---

## Files

- Create: `docs/superpowers/specs/2026-06-21-cache-hit-access-log-wait.md`
- Create: `docs/superpowers/plans/2026-06-21-cache-hit-access-log-wait.md`
- Modify: `crates/ntgw-http/src/runtime/tests_support_helpers.rs`
- Modify: `crates/ntgw-http/src/runtime/tests_http1/connection_and_direct/cache_hit_access_log.rs`

## Task 1: Add RED regression coverage

- [x] Add this async regression test to `crates/ntgw-http/src/runtime/tests_support_helpers.rs`:

```rust
#[tokio::test]
async fn wait_for_log_contents_matching_waits_for_predicate() {
    let log_path = temp_log_path("wait-for-log-contents-matching");
    fs::write(&log_path, "text/plain 200\n").expect("initial log write");
    let writer_path = log_path.clone();
    let writer = tokio::spawn(async move {
        sleep(Duration::from_millis(25)).await;
        fs::write(&writer_path, "text/plain 200\ntext/plain -\n").expect("matching log write");
    });

    let log_contents =
        wait_for_log_contents_matching(&log_path, |contents| contents.contains("text/plain -"))
            .await;

    writer.await.expect("log writer task");
    assert!(log_contents.contains("text/plain 200"));
    assert!(log_contents.contains("text/plain -"));
    let _ = fs::remove_file(log_path);
}
```

- [x] Change the final read in `cache_hit_preserves_sent_response_access_log_headers` to call `wait_for_log_contents_matching`:

```rust
let log_contents = wait_for_log_contents_matching(&log_path, |contents| {
    contents.contains("text/plain 200") && contents.contains("text/plain -")
})
.await;
```

- [x] Run the focused helper test to verify RED:

```bash
cargo test -p ntgw-http wait_for_log_contents_matching_waits_for_predicate -- --nocapture
```

Expected result: compile fails because `wait_for_log_contents_matching` is not defined yet.

## Task 2: Implement predicate-based log waiting

- [x] Add this helper in `crates/ntgw-http/src/runtime/tests_support_helpers.rs`:

```rust
async fn wait_for_log_contents_matching(
    path: &PathBuf,
    predicate: impl Fn(&str) -> bool,
) -> String {
    let mut last_contents = String::new();
    for _ in 0..20 {
        if let Ok(contents) = fs::read_to_string(path) {
            if predicate(&contents) {
                return contents;
            }
            last_contents = contents;
        }
        sleep(Duration::from_millis(50)).await;
    }

    if !last_contents.is_empty() {
        return last_contents;
    }

    fs::read_to_string(path).expect("access log file")
}
```

- [x] Refactor the existing helper to keep its original behavior:

```rust
async fn wait_for_log_contents(path: &PathBuf) -> String {
    wait_for_log_contents_matching(path, |contents| !contents.trim().is_empty()).await
}
```

- [x] Run focused verification:

```bash
cargo test -p ntgw-http wait_for_log_contents_matching_waits_for_predicate -- --nocapture
cargo test -p ntgw-http runtime::tests::cache_hit_preserves_sent_response_access_log_headers -- --nocapture
```

Expected result: both commands pass.

## Task 3: Run acceptance verification and commit

- [x] Run the repository acceptance commands:

```bash
cargo test -p ntgw-http --lib
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Expected result: every command exits 0.

- [x] Confirm unrelated repositories remain unchanged:

```bash
for repo in /root/nantian-gw/gateway /root/nantian-gw/dashboard /root/nantian-gw/website /root/nantian-gw/proto /root/nantian-gw/helm-charts; do
  git -C "$repo" status --short
done
```

Expected result: no new changes caused by this task. Pre-existing unrelated changes may still appear.

- [ ] Commit the fix from the dataplane worktree:

```bash
git add docs/superpowers/specs/2026-06-21-cache-hit-access-log-wait.md \
  docs/superpowers/plans/2026-06-21-cache-hit-access-log-wait.md \
  crates/ntgw-http/src/runtime/tests_support_helpers.rs \
  crates/ntgw-http/src/runtime/tests_http1/connection_and_direct/cache_hit_access_log.rs
git commit -m "test(http): wait for cache hit access log line"
```

## Execution Record

- `cargo test -p ntgw-http runtime::tests::cache_hit_preserves_sent_response_access_log_headers -- --nocapture` before the fix: exit 0 locally after first worktree compile; CI run `27902194981` was the RED failure for this race.
- `cargo test -p ntgw-http wait_for_log_contents_matching_waits_for_predicate -- --nocapture` after adding the RED test and call site: exit 101 with `cannot find function wait_for_log_contents_matching in this scope`.
- `cargo test -p ntgw-http wait_for_log_contents_matching_waits_for_predicate -- --nocapture` after helper implementation: exit 0, 1 passed.
- `cargo test -p ntgw-http runtime::tests::cache_hit_preserves_sent_response_access_log_headers -- --nocapture` after helper implementation: exit 0, 1 passed.
- `cargo test -p ntgw-http --lib`: exit 0, 328 passed.
- `cargo test --workspace`: exit 0; all workspace unit tests, integration tests, and doctests passed.
- `cargo clippy --workspace -- -D warnings`: exit 0.
- `cargo fmt --all -- --check`: exit 0.
- `git diff --check`: exit 0.
- Sibling repository status check: no new changes from this task; pre-existing untracked files remain in `gateway`, and pre-existing `.astro` modifications remain in `website`.
