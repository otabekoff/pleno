# Multi-stage Dockerfile for Pleno
# Build stage: compile Pleno from source
FROM rust:1.82-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build release binaries
RUN cargo build --release --workspace

# Runtime stage: minimal image with just the binaries
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -s /bin/bash pleno

# Copy binaries from builder
COPY --from=builder /app/target/release/pleno /usr/local/bin/pleno
COPY --from=builder /app/target/release/plm /usr/local/bin/plm

# Set ownership
RUN chown pleno:pleno /usr/local/bin/pleno /usr/local/bin/plm

# Create workspace directory
WORKDIR /workspace
RUN chown pleno:pleno /workspace

# Switch to non-root user
USER pleno

# Default command: start REPL
CMD ["pleno", "repl"]

# Labels for container metadata
LABEL org.opencontainers.image.title="Pleno"
LABEL org.opencontainers.image.description="Pleno Programming Language Runtime"
LABEL org.opencontainers.image.source="https://github.com/otabekoff/pleno"
LABEL org.opencontainers.image.licenses="MIT"
