# NOTE: For production deployments, pin base images to a digest (e.g., debian:trixie-slim@sha256:...)
# The ARG defaults below allow CI to override, but pinned digests prevent supply-chain attacks.
# Use Renovate/Dependabot to auto-update digest references in CI.
ARG RUST_IMAGE=docker.io/library/rust:1.96-bookworm
ARG RUNTIME_IMAGE=docker.io/library/debian:trixie-slim

FROM ${RUST_IMAGE} AS chef

ENV CARGO_HOME=/usr/local/cargo

RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake pkg-config clang make g++ \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked

WORKDIR /src/dataplane

FROM chef AS planner

COPY dataplane/ /src/dataplane/
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

COPY --from=planner /src/dataplane/recipe.json recipe.json

ARG DATAPLANE_CARGO_FEATURES=allocator-jemalloc
RUN if [ -n "${DATAPLANE_CARGO_FEATURES}" ]; then \
      cargo chef cook --release --recipe-path recipe.json -p ntgw-app --features "${DATAPLANE_CARGO_FEATURES}"; \
    else \
      cargo chef cook --release --recipe-path recipe.json -p ntgw-app; \
    fi

COPY dataplane/ /src/dataplane/
RUN if [ -n "${DATAPLANE_CARGO_FEATURES}" ]; then \
      cargo build --release -p ntgw-app --features "${DATAPLANE_CARGO_FEATURES}"; \
    else \
      cargo build --release -p ntgw-app; \
    fi

FROM ${RUNTIME_IMAGE}

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/dataplane/target/release/ntgw-app /usr/local/bin/ntgw-app

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:19080/health || exit 1

USER 65532

ENTRYPOINT ["/usr/local/bin/ntgw-app"]
