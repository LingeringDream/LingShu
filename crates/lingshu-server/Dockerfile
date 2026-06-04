# Stage 1: Build
FROM rust:1.88-bookworm AS builder

WORKDIR /app
COPY Cargo.toml rust-toolchain.toml ./
COPY crates/ crates/

RUN cargo build --release -p lingshu-server

# Stage 2: Runtime
FROM gcr.io/distroless/cc-debian12

COPY --from=builder /app/target/release/lingshu-server /usr/local/bin/lingshu-server

EXPOSE 8080

ENTRYPOINT ["lingshu-server"]
