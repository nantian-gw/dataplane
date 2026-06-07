# Security Policy

## Reporting a Vulnerability

**Do not open a public issue.** Instead, please report security vulnerabilities privately to the maintainers.

We will acknowledge your report within 48 hours and provide a timeline for resolution within 5 business days.

## Supported Versions

| Version | Supported |
|---|---|
| Latest `main` | ✅ |
| Latest release | ✅ |
| Older releases | ❌ |

## Security Model

Nantian Gateway Data Plane is a high-performance proxy runtime. Security considerations include:

- **TLS termination**: Managed via `ntgw-shared-tls`, supporting certificate rotation and multiple frontends
- **External authentication**: Via `ntgw-http` ext-authz filter, delegating to external auth services
- **Wasm sandboxing**: Plugins run in wasmtime's sandboxed environment
- **Input validation**: All xDS configuration from the control plane is validated before application

If you discover a vulnerability in any of these areas, please report it immediately.