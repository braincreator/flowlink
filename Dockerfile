# ============================================================
# Stage 1: Builder (multi-arch aware)
# ============================================================
FROM --platform=$BUILDPLATFORM golang:1.25-alpine AS builder

RUN apk add --no-cache git ca-certificates tzdata

WORKDIR /build

# Cache module downloads
COPY go.mod go.sum ./
RUN go mod download

# Copy source
COPY . .

# Resolve target platform from Docker buildx
ARG TARGETOS
ARG TARGETARCH

# Build for the target platform
RUN CGO_ENABLED=0 GOOS=${TARGETOS} GOARCH=${TARGETARCH} \
    go build -ldflags="-s -w -extldflags '-static'" \
    -o /build/flowlink-relay ./cmd/relay

# ============================================================
# Stage 2: Runtime
# ============================================================
FROM alpine:latest

RUN apk add --no-cache ca-certificates tzdata

# Create non-root user
RUN adduser -D -u 1001 -g 1001 flowlink

WORKDIR /app

# Copy binary from builder
COPY --from=builder /build/flowlink-relay /app/flowlink-relay

# Expose ports
EXPOSE 8080 8443

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- http://localhost:8080/api/v1/health/live || exit 1

# Run as non-root user
USER flowlink

ENTRYPOINT ["/app/flowlink-relay"]
CMD ["serve"]
