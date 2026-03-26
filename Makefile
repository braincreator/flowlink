VERSION ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "0.1.0")
GIT_COMMIT ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo "dev")
BUILD_DATE ?= $(shell date -u +%Y-%m-%dT%H:%M:%SZ)
LDFLAGS = -ldflags "-X github.com/braincreator/flowlink/pkg/version.Version=$(VERSION) \
            -X github.com/braincreator/flowlink/pkg/version.GitCommit=$(GIT_COMMIT) \
            -X github.com/braincreator/flowlink/pkg/version.BuildDate=$(BUILD_DATE)"

# Таргеты сборки
.PHONY: build build-agent build-relay clean test install run-agent run-relay

# Сборка всего
build: build-agent build-relay

# Сборка агента
build-agent:
	@echo "Building flowlink agent..."
	go build $(LDFLAGS) -o bin/flowlink ./cmd/agent

# Сборка реле
build-relay:
	@echo "Building flowlink relay..."
	go build $(LDFLAGS) -o bin/flowlink-relay ./cmd/relay

# Кросс-компиляция (для релиза)
build-release:
	@echo "Building for all platforms..."
	@mkdir -p dist
	GOOS=darwin  GOARCH=amd64 go build $(LDFLAGS) -o dist/flowlink_darwin_amd64 ./cmd/agent
	GOOS=darwin  GOARCH=arm64 go build $(LDFLAGS) -o dist/flowlink_darwin_arm64 ./cmd/agent
	GOOS=linux   GOARCH=amd64 go build $(LDFLAGS) -o dist/flowlink_linux_amd64 ./cmd/agent
	GOOS=linux   GOARCH=arm64 go build $(LDFLAGS) -o dist/flowlink_linux_arm64 ./cmd/agent
	GOOS=windows GOARCH=amd64 go build $(LDFLAGS) -o dist/flowlink_windows_amd64.exe ./cmd/agent
	@echo "Done! Check dist/"

# Тесты
test:
	go test ./...

# Линт
lint:
	go vet ./...

# Установка локально
install: build-agent
	@mkdir -p $(HOME)/.flowlink/bin
	cp bin/flowlink $(HOME)/.flowlink/bin/flowlink
	@echo "Installed to ~/.flowlink/bin/flowlink"

# Запуск агента (dev)
run-agent: build-agent
	./bin/flowlink --version
	./bin/flowlink --init || true
	./bin/flowlink agent start

# Запуск реле (dev)
run-relay: build-relay
	./bin/flowlink-relay --version
	./bin/flowlink-relay --config relay.json --api-token dev-token

# Очистка
clean:
	rm -rf bin/ dist/

# Форматирование
fmt:
	go fmt ./...

# Зависимости
deps:
	go mod tidy
	go mod download
