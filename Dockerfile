FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl python3 libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY target/release/flowlink /usr/local/bin/flowlink
RUN chmod +x /usr/local/bin/flowlink

EXPOSE 8080

ENTRYPOINT ["flowlink"]
CMD ["api", "--addr", "0.0.0.0:8080"]
