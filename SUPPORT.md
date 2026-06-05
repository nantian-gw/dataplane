# Support — Nantian Gateway Data Plane

Use English or Chinese when opening an issue.

## Where to Start

- Build and test: [CONTRIBUTING.md](CONTRIBUTING.md)
- Architecture overview: [README.md](README.md)
- Main project: [Nantian Gateway](../nantian-gw/)

## Issue Forms

- **Bug Report**: Runtime panics, traffic misrouting, Wasm sandbox escapes,
  xDS reconnect failures, memory/resource leaks.
- **Feature Request**: New proxy capabilities, protocol support, admin
  endpoints, observability improvements.
- **Question**: Architecture, usage, debugging, or integration questions.

Security-sensitive reports follow [SECURITY.md](SECURITY.md).

## What to Include

- Affected crate or component
- Rust toolchain version (`rustc --version`)
- Reproduction steps and minimal test case
- Logs or metrics output (without secrets)

Do not include private keys, tokens, certificates, or production configs.