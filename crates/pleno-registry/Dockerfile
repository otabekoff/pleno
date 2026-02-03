# Production Dockerfile for Pleno Registry
FROM rust:1.82-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace configuration and locks
COPY Cargo.toml Cargo.lock ./

# Copy all crates to satisfy workspace dependencies
COPY crates ./crates

# Build only the registry in release mode
RUN cargo build --release -p pleno-registry

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/pleno-registry /usr/local/bin/pleno-registry

# Set up data directory for SQLite
RUN mkdir -p /data && chown -R 1000:1000 /data
VOLUME ["/data"]

# Environment defaults
ENV PLENO_REGISTRY_DB=/data/registry.db
ENV PLENO_REGISTRY_HOST=0.0.0.0
ENV PLENO_REGISTRY_PORT=8080

EXPOSE 8080

# Run as non-root user
USER 1000

CMD ["pleno-registry"]
