ARG RUST_IMAGE=docker.io/library/rust:1.88-bookworm
ARG RUNTIME_IMAGE=docker.io/library/debian:bookworm-slim
FROM ${RUST_IMAGE} AS chef

ENV CARGO_HOME=/usr/local/cargo

# Use aliyun mirror for crates.io in China
RUN mkdir -p ${CARGO_HOME} && \
    printf '[source.crates-io]\nreplace-with = "aliyun"\n[source.aliyun]\nregistry = "sparse+https://mirrors.aliyun.com/crates.io-index/"\n' > ${CARGO_HOME}/config.toml

RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake pkg-config clang make g++ \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

COPY proto/ /src/proto/

WORKDIR /src/dataplane
COPY dataplane/ /src/dataplane/
COPY tests/testdata/ /src/tests/testdata/

ARG DATAPLANE_CARGO_FEATURES=allocator-jemalloc
RUN if [ -n "${DATAPLANE_CARGO_FEATURES}" ]; then \
      cargo build --release -p aeg-app --features "${DATAPLANE_CARGO_FEATURES}"; \
    else \
      cargo build --release -p aeg-app; \
    fi

FROM ${RUNTIME_IMAGE}

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
        curl \
        dnsutils \
        iproute2 \
        netcat-openbsd \
        procps \
        tcpdump \
    && rm -rf /var/lib/apt/lists/*

COPY --from=chef /src/dataplane/target/release/aeg-app /usr/local/bin/aeg-app

ENTRYPOINT ["/usr/local/bin/aeg-app"]