# Roadmap — Nantian Gateway Data Plane

The data plane roadmap aligns with the main [Nantian Gateway
roadmap](../nantian-gw/ROADMAP.md). This file tracks data-plane-specific
milestones.

## Current Status (June 2026)

The data plane is part of the v0.2 (Implementation Claim Baseline)
convergence. Core proxy, xDS, admin API, and Wasm plugin systems are
implemented and integrated.

## Milestones

### v0.2 / Implementation Claim Baseline (in progress)
- HTTP/1.1 and HTTP/2 proxy with Gateway API route resolution.
- xDS control channel with mTLS.
- Admin API for health, metrics, and operational queries.
- Prometheus metrics and Grafana dashboard.
- Wasm plugin runtime with SDK and prebuilt modules.

### v0.3 / Production Evidence Baseline (planned)
- 24h soak testing and node drain resilience.
- Multi-environment performance baseline (p95/p99, RSS/CPU, reload-under-load).
- Production overlay with mTLS, secrets, and admin auth.
- Complete backend TLS verification and certificate rotation.

### v0.4 / Community And Expansion Baseline (post-v0.3)
- HTTP/3 / QUIC downstream support.
- Advanced load balancing and backend selection.
- Formal Wasm plugin marketplace or registry integration.
- Extended observability: OpenTelemetry tracing, structured logging.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development workflow and
[ROADMAP.md](../nantian-gw/ROADMAP.md) for the project-wide roadmap.