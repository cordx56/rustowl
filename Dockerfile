FROM debian:bookworm-slim AS builder
WORKDIR /app
RUN apt-get update && \
    apt-get install -y build-essential curl

COPY . .

RUN ./scripts/build/toolchain cargo build --release

# final image
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && \
    apt-get install -y build-essential ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rustowl /usr/local/bin/
COPY --from=builder /app/target/release/rustowlc /usr/local/bin/

RUN rustowl toolchain install --skip-rustowl-toolchain

ENTRYPOINT ["rustowl"]
