.PHONY: build test check clippy fmt lint clean docker docker-up docker-down website website-dev install run-relay run-agent run-shield release

# Build release binary
build:
	cargo build --release

# Run all tests
test:
	cargo test --lib
	cargo test --doc

# Check compilation (fast)
check:
	cargo check --all-targets

# Clippy lints
clippy:
	cargo clippy --all-targets -- -D warnings

# Format code
fmt:
	cargo fmt

# Full lint (fmt + clippy)
lint: fmt clippy

# Clean build artifacts
clean:
	cargo clean
	rm -rf website/out website/.next

# Docker builds
docker:
	docker build -t flowlink-relay .
	docker build -t flowlink-agent -f Dockerfile.agent .

docker-up:
	docker compose up -d

docker-down:
	docker compose down

# Website
website:
	cd website && npm run build

website-dev:
	cd website && npm run dev

# Install binary
install:
	cargo install --path crates/cli

# Run services (dev mode)
run-relay:
	cargo run --bin flowlink -- relay --config examples/relay.json

run-agent:
	cargo run --bin flowlink -- agent --config examples/agent.json

run-shield:
	cargo run --bin flowlink -- shield

# Release build with version info
release:
	cargo build --release --bin flowlink
	strip target/release/flowlink 2>/dev/null || true
	@ls -lh target/release/flowlink
