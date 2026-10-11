# Stage 1: Build
FROM rust:1.88-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/ crates/
# The benchmark route includes this fixture at compile time.
COPY datasets/ datasets/

RUN cargo build --locked --release -p lingshu-server

# Stage 2: Runtime
FROM gcr.io/distroless/cc-debian12

COPY --from=builder /app/target/release/lingshu-server /usr/local/bin/lingshu-server

EXPOSE 8080

ENTRYPOINT ["lingshu-server"]
