# ============================================================
# Multi-stage Dockerfile for VIRS
# Includes frontend build (SolidJS + Vite + TailwindCSS)
# Optimized for minimal image size and fast builds
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
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Create dummy main to cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src

# Copy actual source code
COPY src/ src/
COPY migrations/ migrations/

# Touch to invalidate the dummy build
RUN touch src/main.rs

# Build the real binary
# Note: strip is already enabled in Cargo.toml [profile.release], no need to strip again
ENV SQLX_OFFLINE=true
RUN cargo build --release

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
COPY --from=backend-builder /build/migrations /app/migrations

# Copy frontend static files
COPY --from=frontend-builder /frontend/dist /app/frontend/dist

# Create non-root user
RUN useradd -m -s /bin/sh virs && \
    chown -R virs:virs /app

USER virs

# Expose port
EXPOSE 8080

# Environment
ENV FRONTEND_DIR=/app/frontend/dist

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/api/health || exit 1

# Entry point
ENTRYPOINT ["/app/virs"]
