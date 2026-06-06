# ============================================================
# Multi-stage Dockerfile for VIRS (Monorepo Architecture)
# Builds frontend (SolidJS + Vite) and Rust backend (workspace)
# Produces a minimal runtime image
# ============================================================

# ---- Stage 1: Build Frontend ----
FROM node:20-alpine AS frontend-builder

WORKDIR /frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# ---- Stage 2: Build Rust Backend ----
FROM rust:slim-bookworm AS backend-builder

WORKDIR /build

# Install build dependencies
# - pkg-config + libssl-dev: openssl (reqwest/tokio-tungstenite)
# - build-essential: C compiler (gcc for native deps)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace root manifest and lock file first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Copy all crate Cargo.toml files (preserving directory structure)
COPY crates/libs/virs-types/Cargo.toml crates/libs/virs-types/Cargo.toml
COPY crates/libs/virs-config/Cargo.toml crates/libs/virs-config/Cargo.toml
COPY crates/libs/virs-utils/Cargo.toml crates/libs/virs-utils/Cargo.toml
COPY crates/libs/virs-models/Cargo.toml crates/libs/virs-models/Cargo.toml
COPY crates/libs/virs-ccxt/Cargo.toml crates/libs/virs-ccxt/Cargo.toml
COPY crates/libs/virs-exchange/Cargo.toml crates/libs/virs-exchange/Cargo.toml
COPY crates/services/virs-market/Cargo.toml crates/services/virs-market/Cargo.toml
COPY crates/services/virs-position/Cargo.toml crates/services/virs-position/Cargo.toml
COPY crates/services/virs-bot/Cargo.toml crates/services/virs-bot/Cargo.toml
COPY crates/services/virs-api/Cargo.toml crates/services/virs-api/Cargo.toml
COPY app/virs-app/Cargo.toml app/virs-app/Cargo.toml

# Create dummy source files for dependency caching.
# Each lib.rs must declare the same sub-modules as the real code,
# with empty stub files for each module, so cargo can resolve the workspace.
RUN mkdir -p crates/libs/virs-types/src && \
    echo "pub mod enums; pub mod market; pub mod position; pub mod bot; pub mod exchange_pe; pub mod config;" > crates/libs/virs-types/src/lib.rs && \
    touch crates/libs/virs-types/src/enums.rs crates/libs/virs-types/src/market.rs crates/libs/virs-types/src/position.rs crates/libs/virs-types/src/bot.rs crates/libs/virs-types/src/exchange_pe.rs crates/libs/virs-types/src/config.rs && \
    mkdir -p crates/libs/virs-config/src && \
    echo "mod app_config;" > crates/libs/virs-config/src/lib.rs && \
    touch crates/libs/virs-config/src/app_config.rs && \
    mkdir -p crates/libs/virs-utils/src && \
    echo "pub mod auth; pub mod crypto;" > crates/libs/virs-utils/src/lib.rs && \
    touch crates/libs/virs-utils/src/auth.rs crates/libs/virs-utils/src/crypto.rs && \
    mkdir -p crates/libs/virs-models/src && \
    echo "pub mod common; pub mod user; pub mod grid; pub mod auto; pub mod trading;" > crates/libs/virs-models/src/lib.rs && \
    touch crates/libs/virs-models/src/common.rs crates/libs/virs-models/src/user.rs crates/libs/virs-models/src/grid.rs crates/libs/virs-models/src/auto.rs crates/libs/virs-models/src/trading.rs && \
    mkdir -p crates/libs/virs-ccxt/src/adapter/binance crates/libs/virs-ccxt/src/adapter/bybit crates/libs/virs-ccxt/src/adapter/okx && \
    echo "pub mod types; pub mod errors; pub mod auth; pub mod adapter; pub mod ws_types;" > crates/libs/virs-ccxt/src/lib.rs && \
    touch crates/libs/virs-ccxt/src/types.rs crates/libs/virs-ccxt/src/errors.rs crates/libs/virs-ccxt/src/auth.rs crates/libs/virs-ccxt/src/ws_types.rs && \
    echo "pub mod binance; pub mod bybit; pub mod okx;" > crates/libs/virs-ccxt/src/adapter/mod.rs && \
    echo "pub mod kline_ws; pub mod order_ws;" > crates/libs/virs-ccxt/src/adapter/binance/mod.rs && \
    touch crates/libs/virs-ccxt/src/adapter/binance/kline_ws.rs crates/libs/virs-ccxt/src/adapter/binance/order_ws.rs && \
    touch crates/libs/virs-ccxt/src/adapter/bybit/mod.rs crates/libs/virs-ccxt/src/adapter/okx/mod.rs && \
    mkdir -p crates/libs/virs-exchange/src && \
    echo "pub mod adapter; pub mod registry; pub mod paper; pub mod pe_adapter;" > crates/libs/virs-exchange/src/lib.rs && \
    touch crates/libs/virs-exchange/src/adapter.rs crates/libs/virs-exchange/src/registry.rs crates/libs/virs-exchange/src/paper.rs crates/libs/virs-exchange/src/pe_adapter.rs && \
    mkdir -p crates/services/virs-market/src && \
    echo "pub mod types; pub mod cache; pub mod aggregator; pub mod gap; pub mod source; pub mod engine;" > crates/services/virs-market/src/lib.rs && \
    touch crates/services/virs-market/src/types.rs crates/services/virs-market/src/cache.rs crates/services/virs-market/src/aggregator.rs crates/services/virs-market/src/gap.rs crates/services/virs-market/src/source.rs crates/services/virs-market/src/engine.rs && \
    mkdir -p crates/services/virs-position/src && \
    echo "pub mod engine; pub mod risk; pub mod tracker; pub mod persistence;" > crates/services/virs-position/src/lib.rs && \
    touch crates/services/virs-position/src/engine.rs crates/services/virs-position/src/risk.rs crates/services/virs-position/src/tracker.rs crates/services/virs-position/src/persistence.rs && \
    mkdir -p crates/services/virs-bot/src/common crates/services/virs-bot/src/grid/utils crates/services/virs-bot/src/auto && \
    echo "pub mod common; pub mod grid; pub mod auto;" > crates/services/virs-bot/src/lib.rs && \
    echo "pub mod ports; pub mod ai_client; pub mod indicators;" > crates/services/virs-bot/src/common/mod.rs && \
    touch crates/services/virs-bot/src/common/ports.rs crates/services/virs-bot/src/common/ai_client.rs crates/services/virs-bot/src/common/indicators.rs && \
    echo "pub mod types; pub mod engine; pub mod worker; pub mod ai; pub mod ports; pub mod utils; pub mod adapters;" > crates/services/virs-bot/src/grid/mod.rs && \
    touch crates/services/virs-bot/src/grid/types.rs crates/services/virs-bot/src/grid/engine.rs crates/services/virs-bot/src/grid/worker.rs crates/services/virs-bot/src/grid/ai.rs crates/services/virs-bot/src/grid/ports.rs crates/services/virs-bot/src/grid/adapters.rs && \
    echo "pub mod holdings; pub mod levels; pub mod prompt;" > crates/services/virs-bot/src/grid/utils/mod.rs && \
    touch crates/services/virs-bot/src/grid/utils/holdings.rs crates/services/virs-bot/src/grid/utils/levels.rs crates/services/virs-bot/src/grid/utils/prompt.rs && \
    echo "pub mod types; pub mod engine; pub mod worker; pub mod ai; pub mod ports; pub mod strategy; pub mod adapters;" > crates/services/virs-bot/src/auto/mod.rs && \
    touch crates/services/virs-bot/src/auto/types.rs crates/services/virs-bot/src/auto/engine.rs crates/services/virs-bot/src/auto/worker.rs crates/services/virs-bot/src/auto/ai.rs crates/services/virs-bot/src/auto/ports.rs crates/services/virs-bot/src/auto/strategy.rs crates/services/virs-bot/src/auto/adapters.rs && \
    mkdir -p crates/services/virs-api/src/handlers && \
    echo "pub mod router; pub mod state; pub mod handlers; pub mod middleware; pub mod ws;" > crates/services/virs-api/src/lib.rs && \
    touch crates/services/virs-api/src/router.rs crates/services/virs-api/src/state.rs crates/services/virs-api/src/middleware.rs crates/services/virs-api/src/ws.rs && \
    echo "pub mod health; pub mod auth; pub mod user; pub mod market; pub mod credentials; pub mod ai_credentials; pub mod dashboard; pub mod ai; pub mod grid; pub mod auto_trade; pub mod paper;" > crates/services/virs-api/src/handlers/mod.rs && \
    touch crates/services/virs-api/src/handlers/health.rs crates/services/virs-api/src/handlers/auth.rs crates/services/virs-api/src/handlers/user.rs crates/services/virs-api/src/handlers/market.rs crates/services/virs-api/src/handlers/credentials.rs crates/services/virs-api/src/handlers/ai_credentials.rs crates/services/virs-api/src/handlers/dashboard.rs crates/services/virs-api/src/handlers/ai.rs crates/services/virs-api/src/handlers/grid.rs crates/services/virs-api/src/handlers/auto_trade.rs crates/services/virs-api/src/handlers/paper.rs && \
    mkdir -p app/virs-app/src/adapters && \
    echo "fn main() {}" > app/virs-app/src/main.rs

# Build dependencies only (cached layer)
# SQLX_OFFLINE prevents sqlx from requiring a live database at compile time
ENV SQLX_OFFLINE=true
RUN cargo build --release -p virs-app 2>&1 || true

# Now copy the actual source code
COPY crates/ crates/
COPY app/ app/

# Touch ALL .rs files to invalidate the dummy build cache
RUN find /build -name "*.rs" -exec touch {} +

# Build the real binary
RUN cargo build --release -p virs-app

# ---- Stage 3: Runtime ----
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=backend-builder /build/target/release/virs /app/virs

# Copy migrations
COPY migrations/ /app/migrations/

# Copy frontend static files
COPY --from=frontend-builder /frontend/dist /app/frontend/dist

# Create non-root user
RUN useradd -m -s /bin/sh virs && \
    chown -R virs:virs /app

USER virs

# Expose port
EXPOSE 8080

# Environment defaults (overridden by docker-compose / .env)
ENV FRONTEND_DIR=/app/frontend/dist

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/health || exit 1

# Entry point
ENTRYPOINT ["/app/virs"]
