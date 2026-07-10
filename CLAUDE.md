# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Primary reference

**`AGENTS.md` is the authoritative guide** for this workspace — read it first. It covers build/test/lint commands, the toolchain pin, the crate dependency map, code conventions, CI jobs, Docker, and known issues. This file only adds what `AGENTS.md` does not cover and flags where it has drifted.

## Quick command reference

```bash
cargo build --workspace                              # build everything
cargo test --workspace                               # all tests
cargo test -p <crate> <test_name>                    # single test in one crate
cargo clippy --workspace -- -D warnings              # lint (must pass in CI)
cargo fmt --all -- --check                           # format check
cargo run --release -p ntgw-app -- --config configs/dataplane/config.yaml   # run
```

Runnable configs live in `configs/dataplane/` (`config.yaml`, `config.production.yaml`) — note `AGENTS.md`/README reference a `configs/ntgw.yaml` path that does not exist.

## Runtime-unwrap governance

Production code is being driven to **zero `unwrap()`/`expect()` in governed source files**. This is enforced, not aspirational:

```bash
scripts/audit-runtime-unwraps.sh            # report current state
scripts/audit-runtime-unwraps.sh --enforce  # fails if governed prod source has unwrap/expect
```

- Scope is production Rust under `crates/*/src/`. **Test code is exempt**: `tests/**`, inline `#[cfg(test)]` items, and files pulled in only via `#[cfg(test)] mod ...;`.
- Progress is tracked per-crate in `docs/runtime-unwrap-*.md`. When touching `ntgw-ai`, `ntgw-stream`, or the "rest crates," prefer `?`/error propagation over `unwrap()`/`expect()` in non-test code.

## Corrections to AGENTS.md (verify against `Cargo.toml` before relying on either)

- **Release profile** is now `lto = "fat"` with `strip = "symbols"` (AGENTS.md still says `lto = "thin"`; the switch landed in commit `cfc7a0f`).
- **Pingora** is pinned to `0.8.1` in the workspace (AGENTS.md text says `0.8.0`).
- **`ntgw-echo`** exists as an in-progress crate under `crates/` but is not yet a workspace member in `Cargo.toml` — it will not build with `--workspace` until added.
