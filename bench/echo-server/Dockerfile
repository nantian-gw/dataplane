FROM rust:1.96-slim AS builder
WORKDIR /build
COPY Cargo.toml main.rs ./
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /build/target/release/echo-server /usr/local/bin/echo-server
EXPOSE 8080
CMD ["echo-server"]