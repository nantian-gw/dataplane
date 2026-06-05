# Governance — Nantian Gateway Data Plane

This sub-project follows the lightweight governance model of the main
[Nantian Gateway](../nantian-gw/) project. See the main
[GOVERNANCE.md](../nantian-gw/GOVERNANCE.md) for the full policy.

## Sub-Project Scope

The data plane is a Rust workspace under `dataplane/`. Governance for
this sub-project is delegated to the data plane maintainers, subject to
the overall project's decision model.

## Decision Principles

- Correctness and stability take priority over feature accumulation.
- Validate at the cheapest level first: `cargo check` → unit tests →
  integration tests → Kind smoke → conformance.
- Changes to `proto/` must be verified against the Go control plane.
- Wasm plugin security boundaries must not be weakened without explicit
  review and risk assessment.

## Change Acceptance

A change must have:
- Clear objective with well-scoped impact.
- Validation matched to its scope (unit, integration, or e2e).
- Documentation and configuration updated together.
- No unexplained compatibility breaks with the control plane.

## Roles

Roles are listed in [MAINTAINERS.md](MAINTAINERS.md). The reviewer-to-maintainer
path follows the main project's governance model.