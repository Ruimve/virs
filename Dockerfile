# ============================================================
# Multi-stage Dockerfile for VIRS (Monorepo Architecture)
# Builds frontend (React 19 + Vite) and Rust backend (workspace)
# Produces a minimal runtime image
# ============================================================

# ---- Stage 1: Build Frontend ----
FROM node:22-alpine AS frontend-builder

WORKDIR /app
# corepack prepare 独立成层：pnpm 二进制仅在版本号变化时重新下载，
# 不会因 package.json/pnpm-lock.yaml 变更而触发重复下载。
RUN corepack enable && corepack prepare pnpm@11.22.0 --activate

# Copy pnpm workspace root files (package.json, pnpm-workspace.yaml, pnpm-lock.yaml)
# and shared configs (eslint, prettier) for lint/format checks
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml ./
COPY .prettierrc.json .prettierignore eslint.config.js ./

# Copy web app package.json (for pnpm to resolve workspace deps)
COPY apps/web/package.json apps/web/

# 对齐 npm 默认值：fetch-timeout 5min、fetch-retries 5 次
RUN pnpm config set fetch-timeout 300000 && \
    pnpm config set fetch-retries 5 && \
    pnpm install --frozen-lockfile

# Copy web app source
COPY apps/web/ apps/web/

# 代码质量检查：ESLint（错误阻断）+ Prettier 格式检查（不一致阻断）
RUN pnpm --filter @virs/web run lint && pnpm --filter @virs/web run format:check
RUN pnpm --filter @virs/web run build

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
COPY apps/server/ apps/server/

# Touch our own crate sources to ensure cargo recompiles them
# (COPY may preserve older timestamps from the host, causing cargo to skip recompilation)
RUN find /build/crates /build/apps/server -name "*.rs" -exec touch {} +

# Build the real binary (incremental: only recompiles changed crates)
# Note: cache mount contents are NOT persisted into the image layer,
# so we copy the final binary out to /build/virs for the runtime stage to COPY.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --release -p virs-app && \
    cp /build/target/release/virs /build/virs

# ---- Stage 4: Runtime (Debian Slim) ----
# debian:bookworm-slim provides:
#   - glibc + libgcc (C runtime for ring/assembly)
#   - CA certificates (installed below)
#   - nonroot user (created below, UID 65532)
# Verified safe: no openssl-sys in Cargo.lock (rustls only),
# no libsqlite3 needed (postgres feature only), talib-rs is pure Rust.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r -g 65532 nonroot \
    && useradd -r -u 65532 -g 65532 -s /usr/sbin/nologin nonroot

WORKDIR /app

# Copy binary from builder (chown to nonroot UID 65532)
COPY --chown=65532:65532 --from=backend-builder /build/virs /app/virs

# Copy migrations
COPY --chown=65532:65532 migrations/ /app/migrations/

# Copy strategy prompt templates (PromptLoader reads from STRATEGIES_DIR)
COPY --chown=65532:65532 strategies/ /app/strategies/

# Copy frontend static files
COPY --chown=65532:65532 --from=frontend-builder /app/apps/web/dist /app/apps/web/dist

USER 65532:65532
# Container health is monitored via docker-compose.yml configuration.

# Expose port
EXPOSE 8080

# Environment defaults (overridden by docker-compose / .env)
ENV FRONTEND_DIR=/app/apps/web/dist

# Entry point
ENTRYPOINT ["/app/virs"]
