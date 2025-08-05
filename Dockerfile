FROM rust:1.88.0-slim-trixie AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

WORKDIR /app

ENV RUSTOWL_RUNTIME_DIRS="/opt/rustowl"

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates=20250419 \
        build-essential=12.12 \
        curl=8.14.1-2 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .

RUN RUSTOWL_TOOLCHAIN && \
    RUSTOWL_TOOLCHAIN="$(./scripts/build/toolchain eval "echo $RUSTOWL_TOOLCHAIN")" && \
    export RUSTOWL_TOOLCHAIN && \
    SYSROOT && \
    SYSROOT="/opt/rustowl/sysroot/${RUSTOWL_TOOLCHAIN}" && \
    export SYSROOT && \
    ./scripts/build/toolchain echo "Installing toolchain..." && \
    HOST_TUPLE && \
    HOST_TUPLE="$(./scripts/build/toolchain rustc --print=host-tuple)" && \
    ./scripts/build/toolchain cargo build --release --all-features --target "${HOST_TUPLE}" && \
    mkdir -p /build-output && \
    cp target/"${HOST_TUPLE}"/release/rustowl /build-output/rustowl && \
    cp target/"${HOST_TUPLE}"/release/rustowlc /build-output/rustowlc && \
    echo "${RUSTOWL_TOOLCHAIN}" > /build-output/toolchain

FROM rust:1.88.0-slim-trixie

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates=20250419 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build-output/rustowl /usr/local/bin/rustowl
COPY --from=builder /build-output/rustowlc /usr/local/bin/rustowlc
COPY --from=builder /opt/rustowl/sysroot /opt/rustowl/sysroot

ENV PATH="/usr/local/bin:${PATH}"
ENV RUSTOWL_RUNTIME_DIRS="/opt/rustowl"

ENTRYPOINT ["rustowl"]
