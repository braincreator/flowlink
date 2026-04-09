# ============================================================
# FlowLink Relay — Multi-stage Rust Dockerfile
# ============================================================

# Stage 1: Build
FROM rust:1.80-alpine AS builder

RUN apk add --no-cache git ca-certificates pkgconf musl-dev

WORKDIR /build

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/
COPY crates/crypto/Cargo.toml crates/crypto/
COPY crates/db/Cargo.toml crates/db/
COPY crates/billing/Cargo.toml crates/billing/
COPY crates/agent/Cargo.toml crates/agent/
COPY crates/relay/Cargo.toml crates/relay/
COPY crates/cli/Cargo.toml crates/cli/
COPY crates/shield/Cargo.toml crates/shield/
COPY crates/k8s/Cargo.toml crates/k8s/
COPY crates/gitops/Cargo.toml crates/gitops/

# Create dummy source files to cache dependencies
RUN mkdir -p crates/core/src && echo "" > crates/core/src/lib.rs
RUN mkdir -p crates/crypto/src && echo "" > crates/crypto/src/lib.rs
RUN mkdir -p crates/db/src && echo "" > crates/db/src/lib.rs
RUN mkdir -p crates/billing/src && echo "" > crates/billing/src/lib.rs
RUN mkdir -p crates/agent/src && echo "" > crates/agent/src/lib.rs
RUN mkdir -p crates/relay/src && echo "" > crates/relay/src/lib.rs
RUN mkdir -p crates/cli/src && echo "fn main() {}" > crates/cli/src/main.rs
RUN mkdir -p crates/shield/src && echo "" > crates/shield/src/lib.rs
RUN mkdir -p crates/k8s/src && echo "" > crates/k8s/src/lib.rs
RUN mkdir -p crates/gitops/src && echo "" > crates/gitops/src/lib.rs

# Build dependencies only (cached layer)
RUN cargo build --release 2>/dev/null || true

# Copy actual source
COPY . .

# Touch source files to invalidate the cache
RUN find crates -name "*.rs" -exec touch {} +

# Build for release (stripped)
RUN cargo build --release --bin flowlink && \
    strip /build/target/release/flowlink

# ============================================================
# Stage 2: Runtime (minimal)
# ============================================================
FROM alpine:3.20

RUN apk add --no-cache ca-certificates tzdata

# Create non-root user
RUN adduser -D -h /home/flowlink -s /sbin/nologin flowlink

WORKDIR /home/flowlink

# Copy binary from builder
COPY --from=builder /build/target/release/flowlink /usr/local/bin/flowlink

# Copy default config
COPY crates/cli/examples/relay.json /etc/flowlink/relay.json 2>/dev/null || true

# Create data directory
RUN mkdir -p /home/flowlink/.flowlink && chown -R flowlink:flowlink /home/flowlink

USER flowlink

EXPOSE 8080 8443

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD wget -qO- http://localhost:8080/healthz || exit 1

ENTRYPOINT ["flowlink"]
CMD ["relay", "--config", "/etc/flowlink/relay.json"]
