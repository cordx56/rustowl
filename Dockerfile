FROM rust:1.88.0-slim-trixie AS builder

WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates=20250419 \
        curl=8.14.1-2 && \
    rm -rf /var/lib/apt/lists/*

COPY . .

RUN ./scripts/build/toolchain echo "Installing toolchain..." && \
    HOST_TUPLE="$(./scripts/build/toolchain rustc --print=host-tuple)" && \
    TOOLCHAIN="$(./scripts/build/toolchain eval "echo \$RUSTOWL_TOOLCHAIN")" && \
    ./scripts/build/toolchain cargo build --release --all-features --target "${HOST_TUPLE}" && \
    mkdir -p /build-output && \
    cp target/"${HOST_TUPLE}"/release/rustowl /build-output/rustowl && \
    cp target/"${HOST_TUPLE}"/release/rustowlc /build-output/rustowlc && \
    echo "${TOOLCHAIN}" > /build-output/toolchain

FROM rust:1.88.0-slim-trixie

ENV RUSTOWL_RUNTIME_DIRS="/opt/rustowl"

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates=20250419 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /build-output/rustowl /usr/local/bin/rustowl
COPY --from=builder /build-output/rustowlc /usr/local/bin/rustowlc
COPY --from=builder /build-output/toolchain /tmp/toolchain

RUN TOOLCHAIN="$(cat /tmp/toolchain)" && \
    /usr/local/bin/rustowl toolchain install --path /opt/rustowl/sysroot/"${TOOLCHAIN}" --skip-rustowl-toolchain && \
    rm /tmp/toolchain

ENV PATH="/usr/local/bin:${PATH}"

ENTRYPOINT ["rustowl"]
