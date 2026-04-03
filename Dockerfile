FROM golang:1.24-alpine AS builder
RUN apk add --no-cache git ca-certificates
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o /bin/flowlink-relay ./cmd/relay
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o /bin/flowlink-agent ./cmd/agent
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o /bin/flowlink-bot ./cmd/bot

FROM alpine:3.19
RUN apk add --no-cache ca-certificates tzdata
COPY --from=builder /bin/flowlink-relay /usr/local/bin/flowlink-relay
COPY --from=builder /bin/flowlink-agent /usr/local/bin/flowlink-agent
COPY --from=builder /bin/flowlink-bot /usr/local/bin/flowlink-bot
EXPOSE 8080 8443
ENTRYPOINT ["flowlink-relay"]
