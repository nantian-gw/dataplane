# Security Policy — Aether Gateway Data Plane

The data plane is the Rust-based proxy runtime that handles live traffic,
xDS subscriptions, and admin API endpoints. Security issues in this component
can have direct production impact.

## Reporting

Do not disclose unpatched vulnerabilities in public issues.

1. Use GitHub private security reporting if enabled for the repository.
2. If unavailable, contact maintainers privately before public disclosure.
3. For issues that don't involve exploitation details, use a regular issue.

When reporting, include:
- Affected version or commit
- Reproduction steps
- Scope of impact
- Logs or minimal PoC (without secrets)

## Scope (data-plane specific)

- gRPC/xDS control-channel TLS/mTLS configuration defects
- Backend TLS verification and certificate validation errors
- Admin API (`127.0.0.1:19080`) exposure or auth bypass
- Request misrouting, header injection, or connection smuggling
- Wasm plugin sandbox escapes or host-function abuse
- Denial-of-service via connection or stream exhaustion

## Response

This is a pre-production project. Maintainers will acknowledge reports,
assess impact, and schedule fixes. See the main [Aether Gateway
SECURITY.md](../nantian-gw/SECURITY.md) for the full project-wide policy.

Only the `main` branch is guaranteed to be under continuous maintenance.