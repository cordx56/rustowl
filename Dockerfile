FROM debian:bookworm-slim AS builder
WORKDIR /app
RUN apt-get update && \
    apt-get install -y --no-install-recommends build-essential=12.9 ca-certificates=20230311+deb12u1 curl=7.88.1-10+deb12u12 && \
    rm -rf /var/lib/apt/lists/*

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
