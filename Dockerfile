# ========================================================
# Stage 1: Build & Compile Rust Service and Test Runner
# ========================================================
FROM rust:1-bookworm AS builder

WORKDIR /usr/src/app

# Install system dependencies needed for compiling OpenSSL and TLS crates
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency manifests from backend directory
COPY backend/Cargo.toml backend/Cargo.lock ./

# Cache compilation dependencies
RUN mkdir -p src/bin tests && \
    echo "pub fn dummy() {}" > src/lib.rs && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/bin/test_runner.rs && \
    cargo build --release || true && \
    rm -rf src tests

# Copy application source code from backend directory
COPY backend/ .

# Set SQLX_OFFLINE=true for offline compile-time query checking
ENV SQLX_OFFLINE=true

# Touch source files and compile binaries
RUN touch src/lib.rs src/main.rs src/bin/test_runner.rs && \
    cargo build --release --bin eravaya && \
    cargo build --bin test_runner

# Optional test execution during Docker build (default false to allow building in CI/Cloud without live DB)
ARG RUN_TESTS=false
RUN if [ "$RUN_TESTS" = "true" ]; then \
        cargo run --bin test_runner; \
    fi

# ========================================================
# Stage 2: Production Minimal Runtime Image
# ========================================================
FROM debian:bookworm-slim AS runner

WORKDIR /app

# Install runtime SSL, CA certs, and utilities
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    tzdata \
    uuid-runtime \
    && rm -rf /var/lib/apt/lists/*

# Set default environment
ENV TZ=Asia/Kolkata \
    APP_PORT=8089 \
    PORT=8089 \
    RUN_TESTS_ON_STARTUP=false \
    ALERT_ADMIN_EMAIL=kabhiram67@gmail.com \
    RUST_LOG=info

# Copy compiled binaries and runtime files
COPY --from=builder /usr/src/app/target/release/eravaya /app/eravaya
COPY --from=builder /usr/src/app/target/release/test_runner /app/test_runner
COPY --from=builder /usr/src/app/migrations /app/migrations
COPY --from=builder /usr/src/app/scripts /app/scripts
COPY --from=builder /usr/src/app/entrypoint.sh /app/entrypoint.sh
COPY --from=builder /usr/src/app/ca.pem /app/ca.pem

RUN chmod +x /app/entrypoint.sh /app/eravaya /app/test_runner

EXPOSE 8089

ENTRYPOINT ["/app/entrypoint.sh"]
