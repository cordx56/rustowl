FROM rust:1.88.0-slim-trixie AS builder

ENV RUSTC_BOOTSTRAP=1
ENV RUSTOWL_RUNTIME_DIRS="/opt/rustowl"
WORKDIR /app

RUN rustup component add rust-src rustc-dev llvm-tools

COPY . .

# Force 1.88.0
RUN rm rust-toolchain*

RUN HOST_TUPLE="$(rustc --print=host-tuple)" && \
    cargo build --release --all-features --target "$HOST_TUPLE" && \
    mkdir -p artifacts && \
    cp target/"$HOST_TUPLE"/release/rustowl artifacts/rustowl && \
    cp target/"$HOST_TUPLE"/release/rustowlc artifacts/rustowlc && \
    ACTIVE_TOOLCHAIN="$(rustup show active-toolchain | awk '{ print $1 }')" && \
    mkdir -p sysroot/"$ACTIVE_TOOLCHAIN" && \
    cp -r "$(rustc --print=sysroot)"/* sysroot/"$ACTIVE_TOOLCHAIN"/ && \
    find sysroot -type f | grep -v -E '\.(rlib|so|dylib|dll)$' | xargs rm -rf || true && \
    find sysroot -depth -type d -empty -exec rm -rf {} \;

FROM debian:trixie-slim

WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/artifacts/rustowl /usr/local/bin/rustowl
COPY --from=builder /app/artifacts/rustowlc /usr/local/bin/rustowlc

RUN mkdir -p /opt/rustowl
COPY --from=builder /app/sysroot /opt/rustowl

ENV PATH="/usr/local/bin:${PATH}"

ENTRYPOINT ["rustowl"]
