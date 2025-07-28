FROM rust:1.88.0-slim-trixie

ENV RUSTOWL_RUNTIME_DIRS="/opt/rustowl"

WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN rustup component add rust-src rustc-dev llvm-tools

COPY . .

RUN HOST_TUPLE="$(rustc --print=host-tuple)" && \
    TOOLCHAIN="$(rustup show active-toolchain | awk '{ print $1 }')" && \
    ./scripts/build/toolchain cargo build --release --all-features --target "${HOST_TUPLE}" && \
    cp target/"${HOST_TUPLE}"/release/rustowl /usr/local/bin/rustowl && \
    cp target/"${HOST_TUPLE}"/release/rustowlc /usr/local/bin/rustowlc && \
    /usr/local/bin/rustowl toolchain install --path /opt/rustowl/sysroot/"${TOOLCHAIN}" --skip-rustowl-toolchain && \
    rm -rf /app

ENV PATH="/usr/local/bin:${PATH}"

ENTRYPOINT ["rustowl"]
