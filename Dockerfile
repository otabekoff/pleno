# Builds Pleno from source: `docker build -t pleno .`
# (The release images are made from the prebuilt binaries by Dockerfile.release.)

# Build stage. The workspace uses edition 2024, so it needs a current stable Rust.
FROM rust:1-slim-bookworm AS builder

WORKDIR /app

# Everything the build reads: the crates and the Windows metadata their build
# scripts include.
COPY Cargo.toml Cargo.lock ./
COPY build ./build
COPY crates ./crates

RUN cargo build --release --locked -p pleno-cli -p pleno-lsp -p pleno-dap -p pleno-repl

# Runtime stage: the binaries on the same Debian release they were built on.
FROM debian:bookworm-slim

# HTTPS (std:http, plm over Git) needs the CA certificates.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -s /bin/bash pleno

COPY --from=builder /app/target/release/pleno /app/target/release/plm \
    /app/target/release/pleno-lsp /app/target/release/pleno-dap \
    /app/target/release/pleno-repl /usr/local/bin/

WORKDIR /workspace
RUN chown pleno:pleno /workspace

USER pleno

# Default command: start the REPL
CMD ["pleno", "repl"]

LABEL org.opencontainers.image.title="Pleno"
LABEL org.opencontainers.image.description="Pleno Programming Language Runtime"
LABEL org.opencontainers.image.source="https://github.com/otabekoff/pleno"
LABEL org.opencontainers.image.licenses="MIT"
