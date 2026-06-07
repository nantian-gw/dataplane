# Contributing to Nantian Gateway Data Plane

Thanks for your interest in contributing!

## Getting Started

1. **Fork** the repository
2. **Clone** your fork
3. Install Rust 1.88.0 via [rustup](https://rustup.rs):
   ```bash
   rustup toolchain install 1.88.0
   ```
4. Build and test:
   ```bash
   cargo build --workspace
   cargo test --workspace
   ```

## Development Workflow

1. Create a feature branch from `main`
2. Make your changes
3. Ensure all checks pass:
   ```bash
   cargo check --workspace
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   cargo fmt --all -- --check
   ```
4. Commit using [conventional commits](https://www.conventionalcommits.org/):
   ```
   feat(ntgw-http): add request buffering middleware
   fix(ntgw-ai): correct rate limit counter reset
   ```
5. Open a pull request

## Code Style

- Follow `rustfmt` defaults (CI enforces this)
- No `unsafe` code — several crates use `#![forbid(unsafe_code)]`
- Never suppress type errors with `as any` or `@ts-ignore`
- Prefer existing patterns in the codebase

## Testing

- Unit tests go in `#[cfg(test)]` modules within the source file
- Integration tests go in `tests/` directories
- Property-based tests use `proptest`
- Run the full suite before submitting:
  ```bash
  cargo test --workspace
  ```

## License

By contributing, you agree that your contributions will be licensed under the [Apache-2.0](LICENSE) license.