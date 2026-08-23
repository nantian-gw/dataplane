# Data Plane AI Hot Path and Stream Parser Optimization

Date: 2026-08-23

## Scope

This change is limited to the `dataplane` repository.

Sibling component repositories (`gateway`, `proto`, `dashboard`, `website`,
`helm-charts`, `platform-release`) must remain unchanged.

## Problem

Three small performance/correctness issues remain in the data plane hot path:

1. When AI Gateway is globally enabled, HTTP requests on non-AI paths still read
   and buffer the full request body before `AIGatewayFilter::pre_process`
   rejects the path as unsupported.
2. `FormatAdapter::parse_stream_body()` exists for provider-specific stream
   parsing, but AI response post-processing still calls the generic OpenAI-style
   `parse_sse_chunks()` helper.
3. `cache_request_headers_for_filters_and_access_log()` materializes request
   headers twice when filters and access-log header variables are both active.

## Implementation Plan

1. Gate HTTP AI request-body buffering on `ntgw_ai::format::detect_format(path)`
   before reading downstream body bytes.
2. Switch AI stream post-processing to call the selected adapter's
   `parse_stream_body()` method.
3. Add Anthropic and Ollama stream body parsers, with provider-specific tests.
4. Reuse `ctx.request_headers` when caching access-log request headers.
5. Run targeted tests first, then format/clippy acceptance checks.

## Acceptance Criteria

- `cargo test -p ntgw-ai format`
- `cargo test -p ntgw-ai parse_stream_body`
- `cargo test -p ntgw-ai --test format_anthropic`
- `cargo test -p ntgw-ai --test format_ollama`
- `cargo test -p ntgw-ai --test filter_integration`
- `cargo test -p ntgw-http proxy::tests::ai_body_limit`
- `cargo test -p ntgw-http proxy::tests::context::request::cache_access_log_request_headers`
- `cargo fmt --all -- --check`
- `cargo clippy --locked -p ntgw-ai --all-targets -- -D warnings`
- `cargo clippy --locked -p ntgw-http --all-targets -- -D warnings`
- `git status --short` shows only intended `dataplane` worktree files changed.

## Validation Results

- `cargo test -p ntgw-ai format` — passed; note this command filters by test
  name and does not run the new provider parser tests.
- `cargo test -p ntgw-ai parse_stream_body` — passed: 2 selected tests
  (`format_anthropic`, `format_ollama`).
- `cargo test -p ntgw-ai --test format_anthropic` — passed: 8 tests.
- `cargo test -p ntgw-ai --test format_ollama` — passed: 8 tests.
- `cargo test -p ntgw-ai --test filter_integration` — passed: 14 tests.
- First `cargo test -p ntgw-http proxy::tests::ai_body_limit && cargo test -p
  ntgw-http proxy::tests::context::request::cache_access_log_request_headers`
  attempt failed at compile time after re-enabling the orphaned access-log
  request-header tests; fixed missing test re-export, `Arc<str>` expected map
  keys, and a request path borrow crossing downstream body reads.
- `cargo test -p ntgw-http proxy::tests::ai_body_limit` — passed: 5 tests.
- `cargo test -p ntgw-http proxy::tests::context::request::cache_access_log_request_headers`
  — passed: 3 tests.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --locked -p ntgw-ai --all-targets -- -D warnings` — passed.
- First `cargo clippy --locked -p ntgw-http --all-targets -- -D warnings`
  failed on dead production code (`cache_access_log_request_headers_if_needed`);
  fixed by marking the helper test-only.
- `cargo clippy --locked -p ntgw-http --all-targets -- -D warnings` — passed.
