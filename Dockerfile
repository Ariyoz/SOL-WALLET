# ── Stage 1: Build ──────────────────────────────────────────────────────────
# Use latest stable Rust — edition2024 requires 1.85+
FROM rust:latest AS builder

# Cache bust — increment to force full rebuild
ARG CACHE_BUST=4

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy everything needed for the build
COPY Cargo.toml ./
COPY crates/ crates/
COPY patches/ patches/

# Build — without --locked so Cargo resolves fresh compatible versions
RUN cargo build --release --bin solana-wallet-api

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    libsqlite3-0 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/solana-wallet-api ./api
COPY crates/storage/migrations/ ./migrations/

ENV API_HOST=0.0.0.0
ENV RUST_LOG=info
ENV DATABASE_URL=sqlite:./wallet.db
ENV SOLANA_NETWORK=mainnet-beta
ENV SOLANA_RPC_URL=https://api.mainnet-beta.solana.com

EXPOSE 10000

CMD ["./api"]
