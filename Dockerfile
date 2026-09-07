# ── Stage 1: Build ──────────────────────────────────────────────────────────
FROM rust:1.78-slim AS builder

# Install system dependencies needed for Solana crates
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY patches/ patches/

# Build release binary
RUN cargo build --release --bin api

# ── Stage 2: Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    libsqlite3-0 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy compiled binary from builder
COPY --from=builder /app/target/release/api ./api

# Copy migrations (SQLite needs them at runtime)
COPY crates/storage/migrations/ ./migrations/

# Render sets PORT env var — we read it via API_PORT
ENV API_HOST=0.0.0.0
ENV API_PORT=3000
ENV RUST_LOG=info
ENV DATABASE_URL=sqlite:./wallet.db
ENV SOLANA_NETWORK=mainnet-beta
ENV SOLANA_RPC_URL=https://api.mainnet-beta.solana.com

EXPOSE 3000

CMD ["./api"]
