FROM rust:1.88.0-slim-trixie AS chef
WORKDIR /app

COPY scripts/ scripts/

ENV RUSTC_BOOTSTRAP=1

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        curl=8.14.1-2 && \
    rm -rf /var/lib/apt/lists/*

RUN eval "export $(./scripts/build/print-env.sh "$(cat scripts/build/channel)")" && \
    RUSTUP_TOOLCHAIN="$(cat scripts/build/channel)" && \
    cargo install cargo-chef

FROM chef AS planner
ENV RUSTC_BOOTSTRAP=1
COPY . .
RUN eval "export $(./scripts/build/print-env.sh "$(cat scripts/build/channel)")" && \
    RUSTUP_TOOLCHAIN="$(cat scripts/build/channel)" && \
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
RUN eval "export $(./scripts/build/print-env.sh "$(cat scripts/build/channel)")" && \
    RUSTUP_TOOLCHAIN="$(cat scripts/build/channel)" && \
    cargo chef cook --release --recipe-path recipe.json

COPY . .

RUN eval "export $(./scripts/build/print-env.sh "$(cat scripts/build/channel)")" && \
    export SYSROOT="/opt/rustowl/sysroot/${RUSTOWL_TOOLCHAIN}" && \
    RUSTUP_TOOLCHAIN="$(cat scripts/build/channel)" && \
    ./scripts/build/toolchain cargo build --release --all-features --target "${HOST_TUPLE}" && \
    mkdir -p /build-output && \
    cp target/"${HOST_TUPLE}"/release/rustowl /build-output/rustowl && \
    cp target/"${HOST_TUPLE}"/release/rustowlc /build-output/rustowlc

FROM rust:1.88.0-slim-trixie

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates=20250419 curl=8.14.1-2  && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder scripts/ scripts/

COPY --from=builder /build-output/rustowl /usr/local/bin/rustowl
COPY --from=builder /build-output/rustowlc /usr/local/bin/rustowlc

RUN RUSTOWL_TOOLCHAIN="$(./scripts/build/toolchain eval "echo $RUSTOWL_TOOLCHAIN")" && \
    export SYSROOT="/opt/rustowl/sysroot/${RUSTOWL_TOOLCHAIN}" && \
    ./scripts/build/toolchain "Done!"
    rm -rf scripts/

ENV PATH="/usr/local/bin:${PATH}"
ENV RUSTOWL_RUNTIME_DIRS="/opt/rustowl"

ENTRYPOINT ["rustowl"]
