FROM rust:1.88.0-slim as builder

WORKDIR /app

ENV RUSTC_BOOTSTRAP=1
ENV RUSTOWL_RUNTIME_DIRS="/opt/rustowl"

RUN rustup component add rustc-dev rust-src llvm-tools

COPY . .

RUN cargo build --release

RUN mkdir -p sysroot && \
    ACTIVE_TOOLCHAIN=$(rustup show active-toolchain | awk '{ print $1 }') && \
    cp -r "$(rustc --print=sysroot)" "sysroot/$ACTIVE_TOOLCHAIN" && \
    find sysroot -type f | grep -v -E '\.(rlib|so|dylib|dll)$' | xargs rm -rf && \
    find sysroot -depth -type d -empty -exec rm -rf {} \;

FROM debian:bookworm-slim

WORKDIR /app

COPY --from=builder /app/target/release/rustowl /usr/local/bin/rustowl
COPY --from=builder /app/target/release/rustowlc /usr/local/bin/rustowlc
COPY --from=builder /app/sysroot /opt/rustowl/sysroot

ENTRYPOINT ["rustowl"]
