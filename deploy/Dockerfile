# FlowLink Relay — multi-stage production build
# Optimized for Yandex Cloud / Yandex Container Registry

FROM rust:1.85-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/db/Cargo.toml crates/db/Cargo.toml
COPY crates/agent/Cargo.toml crates/agent/Cargo.toml
COPY crates/relay/Cargo.toml crates/relay/Cargo.toml
COPY crates/relay.rs/Cargo.toml crates/relay.rs/Cargo.toml
COPY crates/shield/Cargo.toml crates/shield/Cargo.toml
COPY crates/cli/Cargo.toml crates/cli/Cargo.toml
COPY crates/billing/Cargo.toml crates/billing/Cargo.toml
COPY crates/k8s/Cargo.toml crates/k8s/Cargo.toml
COPY crates/gitops/Cargo.toml crates/gitops/Cargo.toml

# Create dummy main.rs files for dependency caching
RUN mkdir -p crates/core/src && echo "fn main(){}" > crates/core/src/lib.rs
RUN mkdir -p crates/db/src && echo "fn main(){}" > crates/db/src/lib.rs
RUN mkdir -p crates/agent/src && echo "fn main(){}" > crates/agent/src/lib.rs
RUN mkdir -p crates/relay/src && echo "fn main(){}" > crates/relay/src/lib.rs
RUN mkdir -p crates/relay.rs/src && echo "fn main(){}" > crates/relay.rs/src/lib.rs
RUN mkdir -p crates/shield/src && echo "fn main(){}" > crates/shield/src/lib.rs
RUN mkdir -p crates/cli/src && echo "fn main(){}" > crates/cli/src/main.rs
RUN mkdir -p crates/billing/src && echo "fn main(){}" > crates/billing/src/lib.rs
RUN mkdir -p crates/k8s/src && echo "fn main(){}" > crates/k8s/src/lib.rs
RUN mkdir -p crates/gitops/src && echo "fn main(){}" > crates/gitops/src/lib.rs

RUN cargo build --release 2>/dev/null || true

# Copy actual source
COPY . .

# Touch files to invalidate cache for our code only
RUN find crates -name "*.rs" -exec touch {} +

RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -s /bin/bash flowlink

COPY --from=builder /app/target/release/flowlink /usr/local/bin/flowlink
RUN chmod +x /usr/local/bin/flowlink

# Config directory
RUN mkdir -p /etc/flowlink && chown flowlink:flowlink /etc/flowlink

USER flowlink
WORKDIR /etc/flowlink

EXPOSE 9080 9081

ENTRYPOINT ["flowlink"]
CMD ["relay", "--config", "/etc/flowlink/relay.json"]
