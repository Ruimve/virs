# syntax=docker/dockerfile:1.7
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
# 代码质量检查：ESLint（错误阻断）+ Prettier 格式检查（不一致阻断）
RUN npm run lint && npm run format:check
RUN npm run build

# ---- Stage 2: Cargo Chef Planner ----
# Generates recipe.json from workspace Cargo.toml files.
# recipe.json only changes when dependencies change, enabling
# maximal cache hits in the builder stage's `cargo chef cook`.
FROM rust:slim-bookworm AS planner

WORKDIR /build

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-chef --locked

COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Stage 3: Build Rust Backend ----
FROM rust:slim-bookworm AS backend-builder

WORKDIR /build

# Install build dependencies
# libssl-dev removed: all TLS uses rustls (no openssl-sys in Cargo.lock).
# pkg-config + build-essential kept for C compilation of ring/rustls assembly.
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-chef (layer-cached; only rebuilds when base image changes)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-chef --locked

# Cook dependencies from recipe (cached unless Cargo.toml/dependencies change).
# This replaces the manual dummy-file approach: cargo-chef automatically
# generates stub sources from recipe.json and compiles dependencies only.
# SQLX_OFFLINE prevents sqlx from requiring a live database at compile time.
ENV SQLX_OFFLINE=true
COPY --from=planner /build/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json

# Copy real source code (overwrites chef's dummy stubs)
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY app/ app/

# Touch our own crate sources to ensure cargo recompiles them
# (COPY may preserve older timestamps from the host, causing cargo to skip recompilation)
RUN find /build/crates /build/app -name "*.rs" -exec touch {} +

# Build the real binary (incremental: only recompiles changed crates)
# Note: cache mount contents are NOT persisted into the image layer,
# so we copy the final binary out to /build/virs for the runtime stage to COPY.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --release -p virs-app && \
    cp /build/target/release/virs /build/virs

# ---- Stage 4: Runtime (Distroless) ----
# gcr.io/distroless/cc-debian12:nonroot provides:
#   - glibc + libgcc (C runtime for ring/assembly)
#   - CA certificates (for TLS verification)
#   - nonroot user (UID 65532, no shell, no package manager)
# Verified safe: no openssl-sys in Cargo.lock (rustls only),
# no libsqlite3 needed (postgres feature only), talib-rs is pure Rust.
FROM gcr.io/distroless/cc-debian12:nonroot

WORKDIR /app

# Copy binary from builder (chown to distroless nonroot UID 65532)
COPY --chown=65532:65532 --from=backend-builder /build/virs /app/virs

# Copy migrations
COPY --chown=65532:65532 migrations/ /app/migrations/

# Copy frontend static files
COPY --chown=65532:65532 --from=frontend-builder /frontend/dist /app/frontend/dist

# distroless:nonroot already runs as UID 65532 — no USER directive needed.
# No HEALTHCHECK in Dockerfile: distroless has no shell/curl.
# Container health is monitored via docker-compose.yml configuration.

# Expose port
EXPOSE 8080

# Environment defaults (overridden by docker-compose / .env)
ENV FRONTEND_DIR=/app/frontend/dist

# Entry point
ENTRYPOINT ["/app/virs"]
