FROM rust:1.88.0-slim-trixie AS builder

WORKDIR /app

ENV RUSTC_BOOTSTRAP=1
ENV RUSTUP_TOOLCHAIN=${RUST_VERSION}

RUN rustup component add llvm-tools rustc-dev rust-src \
    && cargo install rustowl

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/cargo/bin/rustowl /usr/local/bin/rustowl
COPY --from=builder /usr/local/cargo/bin/rustowlc /usr/local/bin/rustowlc

ENV PATH="/usr/local/bin:${PATH}"

RUN rustowl toolchain install

ENTRYPOINT ["rustowl"]
