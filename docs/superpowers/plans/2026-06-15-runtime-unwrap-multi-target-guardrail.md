# Runtime Unwrap Multi-Target Guardrail Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generalize the dataplane runtime unwrap guardrail to scan both `ntgw-ai` and `ntgw-stream`, add the `ntgw-stream` governance note, and verify the scanner plus `ntgw-stream` crate remain green.

**Architecture:** Keep the existing Rust-source parsing and test-only exclusion logic intact, but refactor target orchestration into an explicit governed-target table inside the embedded Python scanner. Validate note files and batch markers per target before scanning, then render ordered per-target report sections for both `--report` and `--enforce`.

**Tech Stack:** Bash, embedded Python 3, Rust cargo test, git

---

### Task 1: Document The Approved Guardrail Contract

**Files:**
- Create: `docs/runtime-unwrap-ntgw-stream-zero-tolerance.md`
- Modify: `docs/superpowers/specs/2026-06-15-runtime-unwrap-multi-target-guardrail-design.md`

- [ ] **Step 1: Write the governance note exactly as approved**

```markdown
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
```

- [ ] **Step 2: Verify the note contains the required marker**

Run: `rg -n "2026-06-15|ntgw-stream/src/" docs/runtime-unwrap-ntgw-stream-zero-tolerance.md`
Expected: Matches for the date marker and governed source root

### Task 2: Make The Scanner Multi-Target

**Files:**
- Modify: `scripts/audit-runtime-unwraps.sh`
- Test: `scripts/test-audit-runtime-unwraps.py`

- [ ] **Step 1: Run the fixture suite before implementation**

Run: `python3 scripts/test-audit-runtime-unwraps.py`
Expected: FAIL because the current scanner still reports only the single `ntgw-ai` target

- [ ] **Step 2: Replace the single-target Python prelude with the approved governed-target table**

```python
from dataclasses import dataclass
from pathlib import Path
import re
import sys


@dataclass(frozen=True)
class GovernedTarget:
    name: str
    root: Path
    doc: Path
    marker: str


class TargetValidationError(Exception):
    pass


targets = [
    GovernedTarget(
        name="ntgw-ai",
        root=Path("crates/ntgw-ai/src"),
        doc=Path("docs/runtime-unwrap-ntgw-ai-zero-tolerance.md"),
        marker="2026-06-15",
    ),
    GovernedTarget(
        name="ntgw-stream",
        root=Path("crates/ntgw-stream/src"),
        doc=Path("docs/runtime-unwrap-ntgw-stream-zero-tolerance.md"),
        marker="2026-06-15",
    ),
]
```

- [ ] **Step 3: Add the local target scanner helper and keep the existing filtering logic**

```python
def scan_target(root: Path):
    matches = []
    excluded_paths = set()
    excluded_dirs = set()
    files = sorted(root.rglob("*.rs"))
    file_cache = {}

    for path in files:
        lines = path.read_text().splitlines()
        sanitized_lines = sanitize_lines(lines)
        file_cache[path] = (lines, sanitized_lines)
        file_excluded_paths, file_excluded_dirs = collect_cfg_test_module_targets(
            path, lines, sanitized_lines
        )
        excluded_paths.update(file_excluded_paths)
        excluded_dirs.update(file_excluded_dirs)

    for path in files:
        if path in excluded_paths:
            continue
        if any(parent in excluded_dirs for parent in [path, *path.parents]):
            continue
        lines, sanitized_lines = file_cache[path]
        file_matches = production_lines(path, lines, sanitized_lines)
        for lineno, line in file_matches:
            matches.append(f"{path}:{lineno}:{line}")

    return matches
```

- [ ] **Step 4: Replace the bottom control flow with ordered per-target validation and reporting**

```python
try:
    results = []

    for target in targets:
        if not target.doc.is_file():
            raise TargetValidationError(f"{target.doc} is missing")
        if target.marker not in target.doc.read_text():
            raise TargetValidationError(
                f"{target.doc} is missing the {target.marker} batch marker"
            )
        results.append((target, scan_target(target.root)))

    failed = False
    for index, (target, matches) in enumerate(results):
        if index:
            print()
        print(f"== {target.name} ({target.root}) ==")
        if matches:
            failed = True
            print("\n".join(matches))
        else:
            print("clean")

    if failed:
        sys.exit(1)
except TargetValidationError as exc:
    print(f"::error::{exc}", file=sys.stderr)
    sys.exit(1)
except Exception as exc:
    print(f"::error::runtime unwrap scanner failed: {exc}", file=sys.stderr)
    sys.exit(2)
```

- [ ] **Step 5: Keep the shell-level `--report` and `--enforce` status contract intact**

Run: `sed -n '1,260p' scripts/audit-runtime-unwraps.sh`
Expected: Shell wrapper still exposes only `--report` and `--enforce`

### Task 3: Verify The Guardrail And Crate

**Files:**
- Verify: `scripts/audit-runtime-unwraps.sh`
- Verify: `scripts/test-audit-runtime-unwraps.py`
- Verify: `crates/ntgw-stream/**`

- [ ] **Step 1: Re-run the fixture suite**

Run: `python3 scripts/test-audit-runtime-unwraps.py`
Expected: PASS with all listed fixture cases, including the new `ntgw-stream` multi-target cases

- [ ] **Step 2: Run the report mode on the real repository**

Run: `scripts/audit-runtime-unwraps.sh --report`
Expected: Two headers, one for `ntgw-ai` and one for `ntgw-stream`, with clean output if no governed production matches exist

- [ ] **Step 3: Run the enforce mode on the real repository**

Run: `scripts/audit-runtime-unwraps.sh --enforce`
Expected: Exit status `0` and the same per-target clean sections

- [ ] **Step 4: Run the governed crate tests**

Run: `cargo test -p ntgw-stream`
Expected: PASS

- [ ] **Step 5: Confirm repository-local change scope**

Run: `git status --short`
Expected: Only intended `dataplane` files are modified or added in this worktree; no sibling component repositories are touched

- [ ] **Step 6: Commit the completed task**

```bash
git add docs/runtime-unwrap-ntgw-stream-zero-tolerance.md \
        docs/superpowers/specs/2026-06-15-runtime-unwrap-multi-target-guardrail-design.md \
        docs/superpowers/plans/2026-06-15-runtime-unwrap-multi-target-guardrail.md \
        scripts/audit-runtime-unwraps.sh
git commit -m "feat: generalize runtime unwrap guardrail targets"
```
