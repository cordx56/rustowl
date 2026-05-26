FROM debian:bookworm-slim AS chef
WORKDIR /app
RUN apt-get update && \
    apt-get install -y --no-install-recommends build-essential=12.9 ca-certificates=20230311+deb12u1 curl=7.88.1-10+deb12u14 && \
    rm -rf /var/lib/apt/lists/*

COPY scripts/ scripts/
RUN ./scripts/build/toolchain cargo install cargo-chef --locked

FROM chef AS planner
COPY . .
RUN ./scripts/build/toolchain cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN ./scripts/build/toolchain cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN ./scripts/build/toolchain cargo build --release

# final image
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && \
    apt-get install -y --no-install-recommends build-essential=12.9 ca-certificates=20230311+deb12u1 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rustowl /usr/local/bin/
COPY --from=builder /app/target/release/rustowlc /usr/local/bin/

RUN rustowl toolchain install --skip-rustowl-toolchain

ENTRYPOINT ["rustowl"]
