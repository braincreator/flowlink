FROM --platform=linux/amd64 rust:1.85-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl-dev pkg-config cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache deps
COPY Cargo.toml Cargo.lock ./
COPY crates/db/Cargo.toml crates/db/Cargo.toml
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/agent/Cargo.toml crates/agent/Cargo.toml
COPY crates/billing/Cargo.toml crates/billing/Cargo.toml
COPY crates/api/Cargo.toml crates/api/Cargo.toml
COPY crates/cli/Cargo.toml crates/cli/Cargo.toml
COPY crates/relay/Cargo.toml crates/relay/Cargo.toml

# Create dummy source files for dependency caching
RUN mkdir -p crates/db/src && echo "" > crates/db/src/lib.rs \
    && mkdir -p crates/core/src && echo "" > crates/core/src/lib.rs \
    && mkdir -p crates/agent/src && echo "" > crates/agent/src/lib.rs \
    && mkdir -p crates/billing/src && echo "" > crates/billing/src/lib.rs \
    && mkdir -p crates/api/src && echo "" > crates/api/src/lib.rs \
    && mkdir -p crates/cli/src && echo "" > crates/cli/src/main.rs \
    && mkdir -p crates/relay/src && echo "" > crates/relay/src/lib.rs \
    && cargo build --release --bin flowlink 2>/dev/null || true \
    && rm -rf crates/*/src

# Build for real
COPY crates/ crates/
RUN cargo build --release --bin flowlink

# Output
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=0 /app/target/release/flowlink /usr/local/bin/flowlink
ENTRYPOINT ["flowlink"]
