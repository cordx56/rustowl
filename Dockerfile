FROM rust:1.88.0-slim-trixie AS chef
WORKDIR /app

COPY scripts/ scripts/

ENV RUSTC_BOOTSTRAP=1

RUN export "$(./scripts/build/print-env.sh 1.88.0)" && \
    export SYSROOT="/opt/rustowl/sysroot/${RUSTOWL_TOOLCHAIN}" && \
    export RUSTUP_TOOLCHAIN="${RUSTOWL_TOOLCHAIN}" && \
    cargo install cargo-chef

FROM chef AS planner
ENV RUSTC_BOOTSTRAP=1
COPY . .
RUN export "$(./scripts/build/print-env.sh 1.88.0)" && \
    export SYSROOT="/opt/rustowl/sysroot/${RUSTOWL_TOOLCHAIN}" && \
    export RUSTUP_TOOLCHAIN="${RUSTOWL_TOOLCHAIN}" && \
    cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

ENV RUSTC_BOOTSTRAP=1

WORKDIR /app

ENV RUSTOWL_RUNTIME_DIRS="/opt/rustowl"

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates=20250419 \
        build-essential=12.12 \
        curl=8.14.1-2 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN export "$(./scripts/build/print-env.sh 1.88.0)" && \
    export SYSROOT="/opt/rustowl/sysroot/${RUSTOWL_TOOLCHAIN}" && \
    export RUSTUP_TOOLCHAIN="${RUSTOWL_TOOLCHAIN}" && \
    cargo chef cook --release --recipe-path recipe.json

COPY . .

RUN export "$(./scripts/build/print-env.sh 1.88.0)" && \
    export SYSROOT="/opt/rustowl/sysroot/${RUSTOWL_TOOLCHAIN}" && \
    export RUSTUP_TOOLCHAIN="${RUSTOWL_TOOLCHAIN}" && \
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
