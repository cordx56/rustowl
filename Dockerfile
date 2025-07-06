FROM rust:1.88.0-slim-trixie

WORKDIR /app

ENV RUSTC_BOOTSTRAP=1

RUN rustup component add llvm-tools rustc-dev rust-src

RUN cargo install rustowl

RUN rustowl toolchain install

ENTRYPOINT ["rustowl"]
