FROM debian:bookworm-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Rust is expected to be mounted or multi-stage build used
# This is a runtime image — binary is copied in

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates tar zstd \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r flowlink && useradd -r -g flowlink -d /var/lib/flowlink -s /sbin/nologin flowlink

# Install directories
RUN mkdir -p /etc/flowlink /var/lib/flowlink/backups /var/log/flowlink \
    && chown -R flowlink:flowlink /var/lib/flowlink /var/log/flowlink

# Copy binary (built separately)
COPY target/x86_64-unknown-linux-gnu/release/flowlink-agent /usr/local/bin/flowlink-agent
RUN chmod +x /usr/local/bin/flowlink-agent

# Default config (token passed via env)
COPY docker/config.json /etc/flowlink/config.json.tmpl

ENV FLOWLINK_CONFIG=/etc/flowlink/config.json
ENV FLOWLINK_TOKEN=""

EXPOSE 8080

USER flowlink

ENTRYPOINT ["flowlink-agent"]
CMD ["--config", "/etc/flowlink/config.json"]
