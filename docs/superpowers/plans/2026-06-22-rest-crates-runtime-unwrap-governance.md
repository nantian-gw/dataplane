# Rest Crates Runtime Unwrap Governance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend dataplane runtime unwrap governance to every non-`ntgw-ai` crate production source and remove the remaining true production unwrap/expect calls.

**Architecture:** Keep `scripts/audit-runtime-unwraps.sh` as the canonical scanner, expand its governed target list, and improve its test-only path analysis so included test fixtures do not produce false positives. Convert `ntgw-wasm` global singletons from panicking `expect` initialization to result-returning initialization that the HTTP runtime can log and bypass.

**Tech Stack:** Bash wrapper, embedded Python scanner fixtures, Rust 2024, `std::sync::OnceLock`, `anyhow`, `thiserror`, Cargo workspace tests.

---

### Task 1: Add Scanner Fixture Coverage

**Files:**
- Modify: `scripts/test-audit-runtime-unwraps.py`

- [ ] **Step 1: Add failing fixture cases**

Add these fixture cases to the `cases` list:

```python
        Case(
            name="cfg_all_test_inline_module_excluded",
            files={
                "crates/ntgw-ai/src/lib.rs": (
                    "#[cfg(all(test, target_os = \"linux\"))]\n"
                    "mod tests {\n"
                    "    fn helper() { let _ = Some(1).expect(\"test only\"); }\n"
                    "}\n"
                    "fn production_probe() { let _ = Some(2).expect(\"prod\"); }\n"
                )
            },
            report_code=0,
            enforce_code=1,
            report_contains=[
                'crates/ntgw-ai/src/lib.rs:5:fn production_probe() { let _ = Some(2).expect("prod"); }'
            ],
            report_not_contains=["test only"],
        ),
        Case(
            name="cfg_test_include_tree_excluded",
            files={
                "crates/ntgw-ai/src/lib.rs": (
                    "#[cfg(test)]\n"
                    "mod tests;\n"
                    "fn production_probe() { let _ = Some(3).expect(\"prod\"); }\n"
                ),
                "crates/ntgw-ai/src/tests.rs": 'include!("tests_http1.rs");\n',
                "crates/ntgw-ai/src/tests_http1.rs": (
                    'include!("tests_http1/case.rs");\n'
                ),
                "crates/ntgw-ai/src/tests_http1/case.rs": (
                    "fn helper() { let _ = Some(1).unwrap(); }\n"
                ),
            },
            report_code=0,
            enforce_code=1,
            report_contains=[
                'crates/ntgw-ai/src/lib.rs:3:fn production_probe() { let _ = Some(3).expect("prod"); }'
            ],
            report_not_contains=["tests_http1.rs", "tests_http1/case.rs"],
        ),
```

Update `multi_target_clean_repo_reports_each_target` so `report_contains` and
`enforce_contains` include at least:

```python
"== ntgw-http (crates/ntgw-http/src) ==",
"== ntgw-wasm (crates/ntgw-wasm/src) ==",
"== ntgw-xds (crates/ntgw-xds/src) ==",
```

Add an aggregate default doc fixture:

```python
REST_DOC_FIXTURE = """# Rest Crates Runtime Unwrap Governance

Date: 2026-06-22

This note records the zero-tolerance guardrail expansion for all dataplane
production crate roots that were not already governed by the ntgw-ai and
ntgw-stream batches.
"""
```

and include it in `default_docs` as
`docs/runtime-unwrap-rest-crates-zero-tolerance.md`.

- [ ] **Step 2: Run tests and confirm RED**

Run:

```bash
python3 scripts/test-audit-runtime-unwraps.py
```

Expected: FAIL before implementation. The output should show missing expanded
target names or the newly added fixture cases reporting test-only unwraps.

### Task 2: Expand And Correct The Audit Scanner

**Files:**
- Modify: `scripts/audit-runtime-unwraps.sh`
- Create: `docs/runtime-unwrap-rest-crates-zero-tolerance.md`

- [ ] **Step 1: Add aggregate target documentation**

Create `docs/runtime-unwrap-rest-crates-zero-tolerance.md` with:

```markdown
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
```

- [ ] **Step 2: Expand `targets`**

Add one `GovernedTarget` entry for each newly governed crate root. Use
`docs/runtime-unwrap-rest-crates-zero-tolerance.md` and marker `2026-06-22` for
those entries. Keep existing `ntgw-ai` and `ntgw-stream` entries unchanged.

- [ ] **Step 3: Implement conservative cfg parsing**

Replace the exact-only cfg-test matcher with helpers that treat `test` and
`all(test, ...)` as definitely test-only, while leaving `any(test, feature =
"...")` and `not(test)` as production-reachable.

- [ ] **Step 4: Implement recursive include exclusion**

Add an `include!` regex and a helper that starts from excluded test module
files, follows relative string-literal `include!("...")` targets, and adds
those files to the excluded path set before production scanning.

- [ ] **Step 5: Run scanner fixture tests and confirm GREEN**

Run:

```bash
python3 scripts/test-audit-runtime-unwraps.py
```

Expected: PASS, including the new cases.

- [ ] **Step 6: Run audit enforce and confirm only true production hits remain**

Run:

```bash
scripts/audit-runtime-unwraps.sh --enforce
```

Expected before Task 3: FAIL only for:

```text
crates/ntgw-wasm/src/engine.rs
crates/ntgw-wasm/src/plugin.rs
```

### Task 3: Remove Wasm Production Panics

**Files:**
- Modify: `crates/ntgw-wasm/src/engine.rs`
- Modify: `crates/ntgw-wasm/src/plugin.rs`
- Modify: `crates/ntgw-wasm/src/error.rs`
- Modify: `crates/ntgw-http/src/runtime/server.rs`
- Modify: `crates/ntgw-wasm/tests/engine_tests.rs`
- Modify: `crates/ntgw-wasm/tests/plugin_tests.rs`

- [ ] **Step 1: Add failing Rust API tests**

Add to `crates/ntgw-wasm/tests/engine_tests.rs`:

```rust
use std::sync::Arc;
use ntgw_wasm::engine::{global_engine, WasmEngine};

#[test]
fn test_global_engine_returns_reusable_result() -> Result<()> {
    let first = global_engine()?;
    let second = global_engine()?;
    assert!(Arc::ptr_eq(&first, &second));
    Ok(())
}

#[test]
fn test_wasm_engine_global_returns_result() -> Result<()> {
    let first = WasmEngine::global()?;
    let second = WasmEngine::global()?;
    assert!(Arc::ptr_eq(&first.engine, &second.engine));
    Ok(())
}
```

Add to `crates/ntgw-wasm/tests/plugin_tests.rs`:

```rust
use std::sync::Arc;
use ntgw_wasm::plugin::global_plugin_manager;

#[test]
fn test_global_plugin_manager_returns_reusable_result() -> Result<()> {
    let first = global_plugin_manager()?;
    let second = global_plugin_manager()?;
    assert!(Arc::ptr_eq(&first, &second));
    Ok(())
}
```

- [ ] **Step 2: Run tests and confirm RED**

Run:

```bash
cargo test -p ntgw-wasm
```

Expected: FAIL to compile because `global_engine`, `WasmEngine::global`, and
`global_plugin_manager` still return bare values.

- [ ] **Step 3: Change `global_engine` API**

Store `Result<Arc<Engine>, String>` in `GLOBAL_ENGINE`, initialize with
`create_engine().map(Arc::new)`, clone the `Arc` on success, and return
`anyhow::Error` rebuilt from the cached string on failure. Change
`WasmEngine::global()` to return `Result<Self>`.

- [ ] **Step 4: Change `global_plugin_manager` API**

Add this error variant:

```rust
#[error("failed to initialize wasm runtime: {0}")]
RuntimeInit(String),
```

Store `Result<Arc<PluginManager>, String>` in `GLOBAL_PLUGIN_MANAGER`. Initialize
it through `crate::engine::global_engine()` and `PluginManager::new`, clone the
`Arc` on success, and return `WasmError::RuntimeInit` from cached failure text.

- [ ] **Step 5: Update HTTP runtime caller**

In `build_wasm_filter`, handle `global_plugin_manager()` with a `match`. On
error, log:

```rust
tracing::warn!(
    target: "wasm",
    error = %error,
    "failed to initialize wasm plugin manager"
);
return None;
```

Use the unwrapped manager for both the empty-desired unload path and the
diff-and-apply path.

- [ ] **Step 6: Run focused Rust tests and confirm GREEN**

Run:

```bash
cargo test -p ntgw-wasm
```

Expected: PASS.

### Task 4: Final Verification

**Files:**
- All files changed above

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt --all -- --check
```

Expected: PASS.

- [ ] **Step 2: Audit fixtures**

Run:

```bash
python3 scripts/test-audit-runtime-unwraps.py
```

Expected: PASS.

- [ ] **Step 3: Audit enforcement**

Run:

```bash
scripts/audit-runtime-unwraps.sh --enforce
```

Expected: PASS with clean output for all governed targets.

- [ ] **Step 4: Workspace tests**

Run:

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Workspace clippy**

Run:

```bash
cargo clippy --workspace -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Verify unrelated repositories**

Run:

```bash
for repo in gateway proto dashboard website helm-charts; do
  git -C "/root/nantian-gw/$repo" status --short
done
```

Expected: no output caused by this task.
