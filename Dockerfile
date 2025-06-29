FROM rust:1.88.0-slim as builder

WORKDIR /app

ENV RUSTC_BOOTSTRAP=1

RUN rustup component add rustc-dev rust-src llvm-tools

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

COPY --from=builder /app/target/release/rustowl /usr/local/bin/rustowl

ENTRYPOINT ["rustowl"]
