# Maintainers — Nantian Gateway Data Plane

This sub-project follows the maintenance model of the main
[Nantian Gateway](../nantian-gw/) project. See the main
[MAINTAINERS.md](../nantian-gw/MAINTAINERS.md) for the full role
definitions and project-wide maintainers.

## Data Plane Maintainers

| Role | Name | GitHub | Ownership | Timezone | Merge | Release |
| --- | --- | --- | --- | --- | --- | --- |
| Maintainer | Mahmut Abi | [@mahmut-Abi](https://github.com/mahmut-Abi) | `dataplane/`, `proto/` | `Asia/Shanghai` | yes | yes |

## Responsibilities

Data plane maintainers are responsible for:
- Reviewing and merging changes to `dataplane/` and `proto/`.
- Maintaining Rust workspace quality: tests, clippy, deny, fmt.
- Ensuring xDS protocol compatibility with the Go control plane.
- Maintaining Wasm plugin SDK stability and sandbox security.
- Responding to data-plane-specific defects and security issues.

## Reviewer Path

To become a data plane reviewer:
- Demonstrate consistent, high-quality contributions to Rust crates.
- Provide actionable, verifiable code reviews.
- Understand the project's validate-cheapest-first discipline and
  compatibility boundaries.

See [GOVERNANCE.md](GOVERNANCE.md) and the main project's
[MAINTAINERS.md](../nantian-gw/MAINTAINERS.md) for promotion criteria.