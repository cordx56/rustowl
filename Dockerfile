FROM debian:bookworm-slim

ARG TARGETARCH
ENV RUSTOWL_VERSION=0.3.4

WORKDIR /app

RUN apt-get update && apt-get install -y curl tar && rm -rf /var/lib/apt/lists/*

RUN set -e; \
    if [ "$TARGETARCH" = "amd64" ]; then \
        ARCH="x86_64"; \
    elif [ "$TARGETARCH" = "arm64" ]; then \
        ARCH="aarch64"; \
    else \
        echo "Unsupported architecture: $TARGETARCH" && exit 1; \
    fi; \
    FILENAME="rustowl-${ARCH}-unknown-linux-gnu.tar.gz"; \
    curl -L -o "$FILENAME" "https://github.com/cordx56/rustowl/releases/download/v${RUSTOWL_VERSION}/$FILENAME" && \
    tar -xzf "$FILENAME" --strip-components=1 && \
    mv rustowl /usr/local/bin/ && \
    mv rustowlc /usr/local/bin/ && \
    mv sysroot /opt/rustowl/ && \
    rm "$FILENAME"

ENTRYPOINT ["rustowl"]
