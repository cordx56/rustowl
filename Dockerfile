FROM rust:1.88.0-slim-trixie

ENV RUSTC_BOOTSTRAP=1
ENV RUSTOWL_RUNTIME_DIRS="/opt/rustowl"
WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN rustup component add rust-src rustc-dev llvm-tools

COPY . .

# Force 1.88.0
RUN rm rust-toolchain*

RUN HOST_TUPLE="$(rustc --print=host-tuple)" && \
    cargo build --release --all-features --target "${HOST_TUPLE}" && \
    mkdir -p artifacts && \
    cp target/"${HOST_TUPLE}"/release/rustowl /usr/local/bin/rustowl && \
    cp target/"${HOST_TUPLE}"/release/rustowlc /usr/local/bin/rustowlc && \
    /usr/local/bin/rustowl toolchain install --path sysroot/$(rustup show active-toolchain | awk '{ print $1 }') && \
    mkdir -p /opt/rustowl/sysroot && \
    cp -a sysroot/ /opt/rustowl

ENV PATH="/usr/local/bin:${PATH}"

ENTRYPOINT ["rustowl"]
