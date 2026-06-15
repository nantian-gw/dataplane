# Runtime Unwrap Multi-Target Guardrail Design

## Goal

Generalize `scripts/audit-runtime-unwraps.sh` from a single governed
`ntgw-ai` target into an explicit multi-target runtime unwrap guardrail that
also covers `crates/ntgw-stream/src/`, while preserving the existing
production-vs-test filtering behavior.

## Scope

This design covers only the `dataplane` repository worktree at
`/root/.config/superpowers/worktrees/nantian-gw/dataplane-multi-target-unwrap-guardrail`.
It does not change any sibling repositories in the workspace root.

## Requirements

1. The shell wrapper contract remains `--report` and `--enforce`.
2. The embedded Python scanner uses an explicit governed-target table with:
   `ntgw-ai` and `ntgw-stream`.
3. Each governed target validates:
   its governance note file exists and its `2026-06-15` batch marker is
   present.
4. Report output prints one section per governed target using:
   `== <name> (<root>) ==`.
5. `--report` exits successfully after printing all governed target results.
6. `--enforce` exits non-zero when any governed target contains production
   `unwrap` or `expect`.
7. Existing exclusions remain intact:
   inline `#[cfg(test)]` items, files reached only through
   `#[cfg(test)] mod ...;`, and standalone test-only paths outside governed
   production code.
8. Add
   `docs/runtime-unwrap-ntgw-stream-zero-tolerance.md`
   with the exact governance note content supplied by the approved task plan.

## Design

The scanner keeps its current parsing logic for Rust source sanitization,
`#[cfg(test)]` module discovery, and production-line matching. The only
behavioral change is target orchestration: the Python block first defines a
`GovernedTarget` dataclass plus a `targets` list, then iterates that list to
validate governance documents and scan each source root independently.

Target-specific results are accumulated first, then rendered in order with a
blank line between target sections. Any governed target with matches marks the
run as failed for enforcement. Missing governance notes or missing batch
markers are treated as configuration errors and abort with a dedicated
`TargetValidationError`.

## Testing

Acceptance commands for this task are:

- `python3 scripts/test-audit-runtime-unwraps.py`
- `scripts/audit-runtime-unwraps.sh --report`
- `scripts/audit-runtime-unwraps.sh --enforce`
- `cargo test -p ntgw-stream`
- `git status --short`

## Acceptance Criteria

- The fixture suite passes without modifying the intentional Task 2 harness
  beyond any tiny compatibility adjustment that proves strictly necessary.
- Both report and enforce modes print separate `ntgw-ai` and `ntgw-stream`
  sections.
- Production matches in `ntgw-stream` are detected in fixtures, while test-only
  exclusions still hold.
- The new `ntgw-stream` governance note exists verbatim and includes the
  `2026-06-15` marker.
- `cargo test -p ntgw-stream` passes, demonstrating no behavioral regression in
  the governed production crate.
- `git status --short` shows changes only inside this `dataplane` worktree,
  satisfying the repository-specific requirement that unrelated component
  repositories remain unchanged.
