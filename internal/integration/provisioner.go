// Package integration — provisioning контейнеров.
package integration

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"sync"
	"time"
)

// Provisioner — автоматическое создание и настройка контейнеров для клиентов.
type Provisioner struct {
	mu         sync.Mutex
	dockerAPI  string // Docker API socket path
	basePort   int    // starting port for relay containers (default: 9081)
	configDir  string // directory for per-client configs
	dataDir    string // directory for per-client data
	logger     *slog.Logger

	// In-memory state
	clients map[string]*ProvisionedClient // customerID -> client
}

// ProvisioningRequest — запрос на provisioning.
type ProvisioningRequest struct {
	CustomerID     string
	CustomerEmail  string
	PlanID         string
	SubscriptionID string
}

// ProvisioningResult — результат provisioning.
type ProvisioningResult struct {
	ContainerID string
	Port        int
	HealthURL   string
	Credentials *ConnectionCredentials
	SetupTime   time.Duration
}

// ConnectionCredentials — credentials для подключения.
type ConnectionCredentials struct {
	RelayURL     string // wss://relay.flowlink.dev:PORT
	APIToken     string // auto-generated
	ClientID     string
	SetupCommand string // one-liner for client to run
}

// ProvisionedClient — provisioned client info.
type ProvisionedClient struct {
	CustomerID string
	ContainerID string
	Port        int
	Status      string // "running", "stopped", "error"
	CreatedAt   time.Time
	PlanID      string
}

// NewProvisioner — создаёт provisioner.
func NewProvisioner(dockerAPI string, basePort int, configDir, dataDir string, logger *slog.Logger) *Provisioner {
	if logger == nil {
		logger = slog.Default()
	}
	if dockerAPI == "" {
		dockerAPI = "/var/run/docker.sock"
	}
	if basePort == 0 {
		basePort = 9081
	}
	if configDir == "" {
		configDir = "/var/lib/flowlink/config"
	}
	if dataDir == "" {
		dataDir = "/var/lib/flowlink/data"
	}

	// Create directories
	os.MkdirAll(configDir, 0755)
	os.MkdirAll(dataDir, 0755)

	return &Provisioner{
		dockerAPI: dockerAPI,
		basePort:  basePort,
		configDir: configDir,
		dataDir:   dataDir,
		logger:    logger,
		clients:   make(map[string]*ProvisionedClient),
	}
}

// Provision — creates a new relay container for a customer.
// Steps:
// 1. Generate unique client ID and API token (crypto/rand, 32 bytes hex)
// 2. Create per-client config directory
// 3. Create Docker container
// 4. Start container
// 5. Wait for health check (max 60s)
// 6. Generate connection credentials
// 7. Return result
func (p *Provisioner) Provision(ctx context.Context, req *ProvisioningRequest) (*ProvisioningResult, error) {
	p.mu.Lock()
	defer p.mu.Unlock()

	start := time.Now()

	p.logger.Info("starting provisioning", "customer_id", req.CustomerID, "plan_id", req.PlanID)

	// 1. Generate unique client ID and API token
	clientID, err := generateRandomHex(16) // 32 chars
	if err != nil {
		return nil, fmt.Errorf("failed to generate client ID: %w", err)
	}

	apiToken, err := generateRandomHex(32) // 64 chars
	if err != nil {
		return nil, fmt.Errorf("failed to generate API token: %w", err)
	}

	// 2. Create per-client directories
	clientConfigDir := filepath.Join(p.configDir, clientID)
	clientDataDir := filepath.Join(p.dataDir, clientID)
	archiveDir := filepath.Join(p.dataDir, "archive", clientID)

	if err := os.MkdirAll(clientConfigDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create config dir: %w", err)
	}
	if err := os.MkdirAll(clientDataDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create data dir: %w", err)
	}
	if err := os.MkdirAll(archiveDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create archive dir: %w", err)
	}

	// 3. Find next available port
	port := p.basePort + len(p.clients)
	for p.isPortUsed(port) {
		port++
	}

	// 4. Create Docker container via API
	containerID, err := p.createContainer(ctx, clientID, apiToken, port, clientDataDir, req)
	if err != nil {
		return nil, fmt.Errorf("failed to create container: %w", err)
	}

	// 5. Start container
	if err := p.startContainer(ctx, containerID); err != nil {
		return nil, fmt.Errorf("failed to start container: %w", err)
	}

	// 6. Wait for health check (max 60s)
	healthURL := fmt.Sprintf("http://localhost:%d/health", port)
	if err := p.waitForHealth(ctx, healthURL, 60*time.Second); err != nil {
		p.logger.Error("health check failed, stopping container", "container_id", containerID, "err", err)
		p.stopContainer(ctx, containerID, 10*time.Second)
		p.removeContainer(ctx, containerID)
		return nil, fmt.Errorf("health check failed: %w", err)
	}

	// 7. Generate credentials
	relayURL := fmt.Sprintf("wss://relay.flowlink.dev:%d", port)
	setupCommand := fmt.Sprintf("curl -sSL https://get.flowlink.dev/install.sh | CLIENT_ID=%s API_TOKEN=%s bash", clientID, apiToken)

	credentials := &ConnectionCredentials{
		RelayURL:     relayURL,
		APIToken:     apiToken,
		ClientID:     clientID,
		SetupCommand: setupCommand,
	}

	// 8. Store in memory
	p.clients[req.CustomerID] = &ProvisionedClient{
		CustomerID:  req.CustomerID,
		ContainerID: containerID,
		Port:        port,
		Status:      "running",
		CreatedAt:   time.Now(),
		PlanID:      req.PlanID,
	}

	setupTime := time.Since(start)

	p.logger.Info("provisioning completed",
		"customer_id", req.CustomerID,
		"container_id", containerID,
		"client_id", clientID,
		"port", port,
		"setup_time", setupTime,
	)

	return &ProvisioningResult{
		ContainerID: containerID,
		Port:        port,
		HealthURL:   healthURL,
		Credentials: credentials,
		SetupTime:   setupTime,
	}, nil
}

// Deprovision — removes a client's container and cleans up.
// Steps:
// 1. Stop container (with 30s timeout)
// 2. Remove container
// 3. Archive client data (move to archive dir, not delete)
// 4. Remove from traffic router
// 5. Cleanup config
func (p *Provisioner) Deprovision(ctx context.Context, customerID string) error {
	p.mu.Lock()
	defer p.mu.Unlock()

	p.logger.Info("starting deprovisioning", "customer_id", customerID)

	// 1. Get client info
	client, ok := p.clients[customerID]
	if !ok {
		return fmt.Errorf("client %s not found", customerID)
	}

	// 2. Stop container (30s timeout)
	if err := p.stopContainer(ctx, client.ContainerID, 30*time.Second); err != nil {
		p.logger.Error("failed to stop container", "err", err, "container_id", client.ContainerID)
		// Continue anyway
	}

	// 3. Remove container
	if err := p.removeContainer(ctx, client.ContainerID); err != nil {
		p.logger.Error("failed to remove container", "err", err, "container_id", client.ContainerID)
		// Continue anyway
	}

	// 4. Archive client data
	clientDataDir := filepath.Join(p.dataDir, client.ContainerID[:12])
	archiveDir := filepath.Join(p.dataDir, "archive", client.ContainerID[:12])
	if _, err := os.Stat(clientDataDir); err == nil {
		if err := os.Rename(clientDataDir, archiveDir); err != nil {
			p.logger.Error("failed to archive client data", "err", err, "customer_id", customerID)
		}
	}

	// 5. Remove from memory
	delete(p.clients, customerID)

	p.logger.Info("deprovisioning completed", "customer_id", customerID)

	return nil
}

// UpdateResources — updates container CPU/RAM limits (for plan changes).
func (p *Provisioner) UpdateResources(ctx context.Context, customerID string, cpuLimit, memLimit int64) error {
	p.mu.Lock()
	defer p.mu.Unlock()

	client, ok := p.clients[customerID]
	if !ok {
		return fmt.Errorf("client %s not found", customerID)
	}

	// Update container resources via Docker API
	// POST /containers/{id}/update
	updateReq := map[string]interface{}{
		"HostConfig": map[string]interface{}{
			"NanoCpus": cpuLimit,
			"Memory":   memLimit,
		},
	}

	body, err := json.Marshal(updateReq)
	if err != nil {
		return fmt.Errorf("failed to marshal update request: %w", err)
	}

	resp, err := p.dockerRequest(ctx, "POST", fmt.Sprintf("/containers/%s/update", client.ContainerID), body)
	if err != nil {
		return fmt.Errorf("failed to update container: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		respBody, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("docker update failed: %s", string(respBody))
	}

	p.logger.Info("container resources updated", "customer_id", customerID, "cpu", cpuLimit, "mem", memLimit)

	return nil
}

// GetProvisionedClients — list all provisioned clients.
func (p *Provisioner) GetProvisionedClients() ([]ProvisionedClient, error) {
	p.mu.Lock()
	defer p.mu.Unlock()

	result := make([]ProvisionedClient, 0, len(p.clients))
	for _, client := range p.clients {
		result = append(result, *client)
	}
	return result, nil
}

// === Docker API Helpers ===

// createContainer creates a new Docker container
func (p *Provisioner) createContainer(ctx context.Context, clientID, apiToken string, port int, dataDir string, req *ProvisioningRequest) (string, error) {
	// POST /containers/create
	createReq := map[string]interface{}{
		"Image": "ghcr.io/braincreator/flowlink-relay:latest",
		"Env": []string{
			fmt.Sprintf("FLOWLINK_CLIENT_ID=%s", clientID),
			fmt.Sprintf("FLOWLINK_API_TOKEN=%s", apiToken),
			fmt.Sprintf("FLOWLINK_PORT=%d", port),
		},
		"Labels": map[string]string{
			"flowlink.client":  req.CustomerID,
			"flowlink.managed": "true",
			"flowlink.plan":    req.PlanID,
		},
		"HostConfig": map[string]interface{}{
			"PortBindings": map[string]interface{}{
				"8080/tcp": []map[string]interface{}{
					{"HostPort": fmt.Sprintf("%d", port)},
				},
			},
			"Binds": []string{
				fmt.Sprintf("%s:/data", dataDir),
			},
		},
		"ExposedPorts": map[string]interface{}{
			"8080/tcp": struct{}{},
		},
	}

	body, err := json.Marshal(createReq)
	if err != nil {
		return "", fmt.Errorf("failed to marshal create request: %w", err)
	}

	resp, err := p.dockerRequest(ctx, "POST", "/containers/create", body)
	if err != nil {
		return "", fmt.Errorf("docker request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusCreated {
		respBody, _ := io.ReadAll(resp.Body)
		return "", fmt.Errorf("docker create failed: %s", string(respBody))
	}

	var result struct {
		ID string `json:"Id"`
	}
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return "", fmt.Errorf("failed to decode response: %w", err)
	}

	return result.ID, nil
}

// startContainer starts a Docker container
func (p *Provisioner) startContainer(ctx context.Context, containerID string) error {
	resp, err := p.dockerRequest(ctx, "POST", fmt.Sprintf("/containers/%s/start", containerID), nil)
	if err != nil {
		return fmt.Errorf("docker request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent && resp.StatusCode != http.StatusNotModified {
		respBody, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("docker start failed: %s", string(respBody))
	}

	return nil
}

// stopContainer stops a Docker container with timeout
func (p *Provisioner) stopContainer(ctx context.Context, containerID string, timeout time.Duration) error {
	// POST /containers/{id}/stop?t={timeout}
	path := fmt.Sprintf("/containers/%s/stop?t=%d", containerID, int(timeout.Seconds()))
	resp, err := p.dockerRequest(ctx, "POST", path, nil)
	if err != nil {
		return fmt.Errorf("docker request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent && resp.StatusCode != http.StatusNotModified {
		respBody, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("docker stop failed: %s", string(respBody))
	}

	return nil
}

// removeContainer removes a Docker container
func (p *Provisioner) removeContainer(ctx context.Context, containerID string) error {
	// DELETE /containers/{id}?force=true
	path := fmt.Sprintf("/containers/%s?force=true", containerID)
	resp, err := p.dockerRequest(ctx, "DELETE", path, nil)
	if err != nil {
		return fmt.Errorf("docker request failed: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusNoContent {
		respBody, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("docker remove failed: %s", string(respBody))
	}

	return nil
}

// waitForHealth waits for health check endpoint to respond
func (p *Provisioner) waitForHealth(ctx context.Context, healthURL string, timeout time.Duration) error {
	client := &http.Client{Timeout: 2 * time.Second}
	deadline := time.Now().Add(timeout)

	for time.Now().Before(deadline) {
		req, err := http.NewRequestWithContext(ctx, "GET", healthURL, nil)
		if err != nil {
			return err
		}

		resp, err := client.Do(req)
		if err == nil {
			resp.Body.Close()
			if resp.StatusCode == http.StatusOK {
				return nil
			}
		}

		time.Sleep(2 * time.Second)
	}

	return fmt.Errorf("health check timeout")
}

// dockerRequest makes a request to Docker API via unix socket
func (p *Provisioner) dockerRequest(ctx context.Context, method, path string, body []byte) (*http.Response, error) {
	// Create HTTP client with unix socket transport
	client := &http.Client{
		Transport: &http.Transport{
			DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
				return net.Dial("unix", p.dockerAPI)
			},
		},
	}

	var bodyReader io.Reader
	if body != nil {
		bodyReader = io.NopCloser(NewBuffer(body))
	}

	url := fmt.Sprintf("http://localhost%s", path)
	req, err := http.NewRequestWithContext(ctx, method, url, bodyReader)
	if err != nil {
		return nil, err
	}

	req.Header.Set("Content-Type", "application/json")

	return client.Do(req)
}

// isPortUsed checks if port is already allocated
func (p *Provisioner) isPortUsed(port int) bool {
	for _, client := range p.clients {
		if client.Port == port {
			return true
		}
	}
	return false
}

// === Helpers ===

// generateRandomHex generates random hex string of specified byte length
func generateRandomHex(byteLen int) (string, error) {
	bytes := make([]byte, byteLen)
	if _, err := rand.Read(bytes); err != nil {
		return "", err
	}
	return hex.EncodeToString(bytes), nil
}

// Buffer is a simple io.Reader wrapper for []byte
type Buffer struct {
	data []byte
	pos  int
}

// NewBuffer creates a new Buffer
func NewBuffer(data []byte) *Buffer {
	return &Buffer{data: data}
}

// Read implements io.Reader
func (b *Buffer) Read(p []byte) (n int, err error) {
	if b.pos >= len(b.data) {
		return 0, io.EOF
	}
	n = copy(p, b.data[b.pos:])
	b.pos += n
	return n, nil
}
